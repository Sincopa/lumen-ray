import { writable } from 'svelte/store';
import type { VpnService } from './contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../types/models';
import { errorCode, withTimeout, type ErrorCode } from './errors';

const pending = ['connecting', 'disconnecting', 'reconnecting'];

/** The tunnel is up or on its way up, so a server pick switches it. */
export function isActive(status: VpnConnectionState) {
	return (
		status === 'connected' ||
		status === 'connecting' ||
		status === 'reconnecting'
	);
}

/** A server switch may supersede a pending connection, not a disconnect. */
export function canSwitch(status: VpnConnectionState) {
	return status !== 'disconnecting' && status !== 'initializing';
}

/** A connection on its way up can be cancelled from the main button. */
export function canCancel(status: VpnConnectionState) {
	return status === 'connecting' || status === 'reconnecting';
}

/** A superseded request (for example, cancelled by Disconnect) is not a failure. */
function cancelled(cause: unknown) {
	return (
		typeof cause === 'object' &&
		cause !== null &&
		'code' in cause &&
		cause.code === 'CANCELLED'
	);
}

export interface ConnectionSnapshot {
	status: VpnConnectionState;
	info: VpnConnectionInfo | null;
	busy: boolean;
	error: ErrorCode | null;
	/** The server a pending connection is heading to, shown before it connects. */
	target: string | null;
}

export class ConnectionController {
	private unsubscribe?: () => void;
	private eventRevision = 0;
	/**
	 * Every connect, switch and cancel takes a ticket. Only the latest request
	 * settles the screen: an earlier one that the backend superseded finishes
	 * later with CANCELLED and must not overwrite what the user chose since.
	 */
	private request = 0;
	private current: ConnectionSnapshot = {
		status: 'initializing',
		info: null,
		busy: true,
		error: null,
		target: null,
	};

	readonly state = writable(this.current);

	private errorTimer?: ReturnType<typeof setTimeout>;

	constructor(
		private vpn: VpnService,
		private timeoutMs = 30000,
	) {}

	private update(patch: Partial<ConnectionSnapshot>) {
		this.current = { ...this.current, ...patch };
		if (!pending.includes(this.current.status)) this.current.target = null;
		if (patch.error) {
			if (this.errorTimer) clearTimeout(this.errorTimer);
			this.errorTimer = setTimeout(() => {
				if (this.current.error) {
					this.update({ error: null });
				}
			}, 5000);
		} else if (patch.error === null && this.errorTimer) {
			clearTimeout(this.errorTimer);
			this.errorTimer = undefined;
		}
		this.state.set(this.current);
	}

	private async readConnection() {
		if (this.vpn.getSnapshot)
			return withTimeout(this.vpn.getSnapshot(), this.timeoutMs);
		const [status, info] = await withTimeout(
			Promise.all([this.vpn.getStatus(), this.vpn.getConnectionInfo()]),
			this.timeoutMs,
		);
		return { status, info, error: undefined };
	}

	private async reconcileAfterError(cause: unknown) {
		// An IPC timeout is an unknown outcome: reconcile before enabling another connect.
		const error = cancelled(cause) ? null : errorCode(cause);
		try {
			const snapshot = await this.readConnection();
			const reported =
				snapshot.error != null ? errorCode(snapshot.error) : null;
			// The native session wins once it has connected. A client timeout must
			// not stay on screen after that, while a still-pending or failed
			// session keeps the error that reconcile observed.
			this.update({
				status: snapshot.status,
				info: snapshot.info,
				error: snapshot.status === 'connected' ? reported : (reported ?? error),
			});
		} catch {
			this.update({ status: 'error', info: null, error });
		}
	}

	async initialize() {
		this.update({ status: 'initializing', busy: true, error: null });

		try {
			this.unsubscribe?.();
			if (this.vpn.subscribe) {
				this.unsubscribe = await this.vpn.subscribe((snapshot) => {
					this.eventRevision++;
					this.update({
						status: snapshot.status,
						info: snapshot.info,
						busy: pending.includes(snapshot.status),
						error: snapshot.error ? errorCode(snapshot.error) : null,
					});
				});
			}
			const revision = this.eventRevision;
			const { status, info, error } = await this.readConnection();
			if (revision === this.eventRevision)
				this.update({ status, info, error: error ? errorCode(error) : null });
		} catch (e) {
			this.update({ status: 'error', error: errorCode(e) });
		} finally {
			this.update({
				busy: pending.includes(this.current.status),
			});
		}
	}

	/** Re-reads the native state, e.g. when Android brings Lumen back to the front. */
	async refresh() {
		if (this.current.busy) return;
		const revision = this.eventRevision;
		try {
			const { status, info, error } = await this.readConnection();
			if (revision === this.eventRevision && !this.current.busy)
				this.update({ status, info, error: error ? errorCode(error) : null });
		} catch {
			/* The next state event corrects the screen. */
		}
	}

	/** Runs one request; only the latest request settles the screen. */
	private async settle(ticket: number, action: () => Promise<unknown>) {
		try {
			await withTimeout(action(), this.timeoutMs);
			if (ticket !== this.request) return;
			const { status, info } = await this.readConnection();
			if (ticket === this.request) this.update({ status, info });
		} catch (e) {
			if (ticket === this.request) await this.reconcileAfterError(e);
		} finally {
			if (ticket === this.request)
				this.update({ busy: pending.includes(this.current.status) });
		}
	}

	async toggle(serverId: string | null) {
		// The main button cancels a connection that is still on its way up.
		if (canCancel(this.current.status)) {
			return this.cancel();
		}
		if (this.current.busy) {
			return;
		}

		const disconnecting = this.current.status === 'connected';
		if (!disconnecting && !serverId) {
			this.update({ error: 'invalidServer' });
			return;
		}

		const ticket = ++this.request;
		this.update({
			busy: true,
			error: null,
			status: disconnecting ? 'disconnecting' : 'connecting',
			target: disconnecting ? null : serverId,
		});
		await this.settle(ticket, () =>
			disconnecting ? this.vpn.disconnect() : this.vpn.connect(serverId!),
		);
	}

	/**
	 * Stops a pending connection. The backend supersedes the attempt at its next
	 * step and tears down what it started; the screen shows the stop at once.
	 */
	async cancel() {
		if (!canCancel(this.current.status)) return;
		const ticket = ++this.request;
		this.update({
			busy: true,
			error: null,
			status: 'disconnecting',
			info: null,
			target: null,
		});
		await this.settle(ticket, () => this.vpn.disconnect());
	}

	async switchServer(serverId: string) {
		// A pending connection (a mode change reconnecting, an earlier pick) is
		// superseded by the backend; only a disconnect in progress wins.
		if (!canSwitch(this.current.status)) {
			return;
		}
		if (!serverId) {
			this.update({ error: 'invalidServer' });
			return;
		}

		const wasActive = isActive(this.current.status);

		const ticket = ++this.request;
		this.update({
			busy: true,
			error: null,
			status: 'connecting',
			target: serverId,
		});
		await this.settle(ticket, async () => {
			if (wasActive && !this.vpn.getSnapshot) {
				try {
					await withTimeout(this.vpn.disconnect(), this.timeoutMs);
				} catch {
					// A failed disconnect during a server switch is ignored.
				}
			}
			await (this.vpn.connectExact?.(serverId) ?? this.vpn.connect(serverId));
		});
	}
}

import { writable } from 'svelte/store';
import type { VpnService } from '../services/contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../types/models';
import { errorCode, withTimeout, type ErrorCode } from '../services/errors';

const pending = ['connecting', 'disconnecting', 'reconnecting'];

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
}

export class ConnectionController {
	private unsubscribe?: () => void;
	private eventRevision = 0;
	private current: ConnectionSnapshot = {
		status: 'initializing',
		info: null,
		busy: true,
		error: null,
	};

	readonly state = writable(this.current);

	private errorTimer?: ReturnType<typeof setTimeout>;

	constructor(
		private vpn: VpnService,
		private timeoutMs = 30000,
	) {}

	private update(patch: Partial<ConnectionSnapshot>) {
		this.current = { ...this.current, ...patch };
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

	async toggle(serverId: string | null) {
		if (this.current.busy) {
			return;
		}

		const disconnecting = this.current.status === 'connected';
		if (!disconnecting && !serverId) {
			this.update({ error: 'invalidServer' });
			return;
		}

		this.update({
			busy: true,
			error: null,
			status: disconnecting ? 'disconnecting' : 'connecting',
		});

		try {
			await withTimeout(
				disconnecting ? this.vpn.disconnect() : this.vpn.connect(serverId!),
				this.timeoutMs,
			);

			const { status, info } = await this.readConnection();

			this.update({ status, info });
		} catch (e) {
			await this.reconcileAfterError(e);
		} finally {
			this.update({
				busy: pending.includes(this.current.status),
			});
		}
	}

	async switchServer(serverId: string) {
		if (this.current.busy) {
			return;
		}
		if (!serverId) {
			this.update({ error: 'invalidServer' });
			return;
		}

		const wasActive =
			this.current.status === 'connected' ||
			this.current.status === 'connecting';

		this.update({
			busy: true,
			error: null,
			status: 'connecting',
		});

		try {
			if (wasActive && !this.vpn.getSnapshot) {
				try {
					await withTimeout(this.vpn.disconnect(), this.timeoutMs);
				} catch {
					// Ignore disconnect error during seamless switch
				}
			}

			await withTimeout(
				this.vpn.connectExact?.(serverId) ?? this.vpn.connect(serverId),
				this.timeoutMs,
			);

			const { status, info } = await this.readConnection();

			this.update({ status, info });
		} catch (e) {
			await this.reconcileAfterError(e);
		} finally {
			this.update({
				busy: pending.includes(this.current.status),
			});
		}
	}
}

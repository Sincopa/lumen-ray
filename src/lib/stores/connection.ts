import { writable } from 'svelte/store';
import type { VpnService } from '../services/contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../types/models';
import { errorCode, withTimeout, type ErrorCode } from '../services/errors';

export interface ConnectionSnapshot {
	status: VpnConnectionState;
	info: VpnConnectionInfo | null;
	busy: boolean;
	error: ErrorCode | null;
}

export class ConnectionController {
	private current: ConnectionSnapshot = {
		status: 'initializing',
		info: null,
		busy: true,
		error: null,
	};

	readonly state = writable(this.current);

	constructor(
		private vpn: VpnService,
		private timeoutMs = 10000,
	) {}

	private update(patch: Partial<ConnectionSnapshot>) {
		this.current = { ...this.current, ...patch };
		this.state.set(this.current);
	}

	private async readConnection() {
		const [status, info] = await withTimeout(
			Promise.all([this.vpn.getStatus(), this.vpn.getConnectionInfo()]),
			this.timeoutMs,
		);
		return { status, info };
	}

	private async reconcileAfterError(cause: unknown) {
		// An IPC timeout is an unknown outcome: reconcile before enabling another connect.
		const error = errorCode(cause);
		try {
			const { status, info } = await this.readConnection();
			this.update({ status, info, error });
		} catch {
			this.update({ status: 'error', info: null, error });
		}
	}

	async initialize() {
		this.update({ status: 'initializing', busy: true, error: null });

		try {
			const { status, info } = await this.readConnection();
			this.update({ status, info });
		} catch (e) {
			this.update({ status: 'error', error: errorCode(e) });
		} finally {
			this.update({ busy: false });
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
			this.update({ busy: false });
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
			if (wasActive) {
				try {
					await withTimeout(this.vpn.disconnect(), this.timeoutMs);
				} catch {
					// Ignore disconnect error during seamless switch
				}
			}

			await withTimeout(this.vpn.connect(serverId), this.timeoutMs);

			const { status, info } = await this.readConnection();

			this.update({ status, info });
		} catch (e) {
			await this.reconcileAfterError(e);
		} finally {
			this.update({ busy: false });
		}
	}
}

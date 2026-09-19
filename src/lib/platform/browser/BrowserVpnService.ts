import type { VpnService } from '../../services/contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../../types/models';
import { ServiceError } from '../../services/errors';
export class BrowserVpnService implements VpnService {
	private state: VpnConnectionState = 'disconnected';
	private info: VpnConnectionInfo | null = null;
	private generation = 0;
	constructor(private delay = 600) {}
	async connect(serverId: string): Promise<void> {
		if (!serverId || serverId.length > 128)
			throw new ServiceError('invalidServer');
		if (this.state === 'connecting' || this.state === 'connected')
			throw new ServiceError('unavailable');
		const generation = ++this.generation;
		this.state = 'connecting';
		await new Promise((resolve) => setTimeout(resolve, this.delay));
		if (generation !== this.generation) return;
		this.info = {
			serverId,
			ip: '',
			protocol: 'Vless',
			connectedAt: Date.now(),
			demo: true,
		};
		this.state = 'connected';
	}
	async disconnect() {
		++this.generation;
		this.state = 'disconnected';
		this.info = null;
	}
	async reconnect() {
		const id = this.info?.serverId;
		if (!id) throw new ServiceError('invalidServer');
		await this.disconnect();
		await this.connect(id);
	}
	async getStatus() {
		return this.state;
	}
	async getConnectionInfo() {
		return this.info ? { ...this.info } : null;
	}
}

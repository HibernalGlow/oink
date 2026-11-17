import type { DeviceOS } from '../devices/types';
export const DEFAULT_PORT = 47470;
export interface Peer {
    id: string;
    device: string;
    compatible: boolean | null;
    problem?: string;
    name: string;
    os: DeviceOS;
    ip: string;
    port: number;
    deviceId: string;
    source?: number;
}
export interface IncomingRequest extends Peer {
    revision: string;
    targetCount: number | null;
    sourceCount: number | null;
}
export const formatAddress = (p: Pick<Peer, 'ip' | 'port'>) => `${p.ip}:${p.port}`;
export function isIPv4(s: string) {
    const parts = s.split('.');
    return parts.length === 4 && parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255);
}

export type SightingSource = 'lan' | 'tailscale' | 'bluetooth';

export function detectSightingSource(ip?: string, source?: number): SightingSource {
    if (source === 3) return 'tailscale';
    if (source === 4) return 'bluetooth';
    if (ip) {
        const cleanIp = ip.split(':')[0];
        const parts = cleanIp.split('.').map(Number);
        if (parts.length === 4 && !parts.some(isNaN)) {
            // Tailscale CGNAT 100.64.0.0/10 (100.64.0.0 – 100.127.255.255)
            if (parts[0] === 100 && parts[1] >= 64 && parts[1] <= 127) {
                return 'tailscale';
            }
            // Bluetooth PAN (e.g. 172.20.10.0/28)
            if (parts[0] === 172 && parts[1] === 20 && parts[2] === 10) {
                return 'bluetooth';
            }
        }
    }
    return 'lan';
}

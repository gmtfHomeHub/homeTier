import type { Space } from '../types';

export const getSpaceIp = (space: Space): string | undefined => {
  if (space.virtual_ip) return space.virtual_ip;
  if (!space.config_json) return undefined;
  try {
    const parsed = JSON.parse(space.config_json);
    return parsed.virtual_ipv4 || undefined;
  } catch {
    return undefined;
  }
};

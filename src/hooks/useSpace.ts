import { Space } from "../types";
import { useSpaceStore } from "../stores/spaceStore";

interface SpaceHook {
  space: Space | null;
  loading: boolean;
  error: string | null;
}

export const useSpace = (spaceId: string): SpaceHook => {
  const spaces = useSpaceStore((s) => s.spaces);
  const space = spaces.find((s) => s.id === spaceId) || null;

  return { space, loading: false, error: null };
};
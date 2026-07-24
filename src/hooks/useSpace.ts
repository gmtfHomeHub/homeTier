import { useState, useEffect } from "react";
import { Space } from "../types";
import { getSpace, getSpaceConfig } from "../utils/api";

interface SpaceHook {
  space: Space | null;
  loading: boolean;
  error: string | null;
}

export const useSpace = (spaceId: string): SpaceHook => {
  const [space, setSpace] = useState<Space | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchSpace = async () => {
      try {
        setLoading(true);
        const spaceData = await getSpace(spaceId);
        setSpace(spaceData);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to fetch space");
        setSpace(null);
      } finally {
        setLoading(false);
      }
    };

    fetchSpace();
  }, [spaceId]);

  return { space, loading, error };
};
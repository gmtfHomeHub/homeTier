import { useState, useCallback } from "react";
import { useSpaceStore } from "../stores/spaceStore";

export function useSpaceConnect() {
  const { connectSpace, disconnectSpace } = useSpaceStore();
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const [disconnectingId, setDisconnectingId] = useState<string | null>(null);

  const connect = useCallback(async (spaceId: string) => {
    setConnectingId(spaceId);
    try {
      await connectSpace(spaceId);
    } catch (e) {
      alert(String(e));
      throw e;
    } finally {
      setConnectingId(null);
    }
  }, [connectSpace]);

  const disconnect = useCallback(async (spaceId: string) => {
    setDisconnectingId(spaceId);
    try {
      await disconnectSpace(spaceId);
    } catch (e) {
      alert(String(e));
      throw e;
    } finally {
      setDisconnectingId(null);
    }
  }, [disconnectSpace]);

  return { connectingId, disconnectingId, connect, disconnect };
}

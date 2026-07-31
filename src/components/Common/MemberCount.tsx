import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Users } from "lucide-react";
import { getSpacePeers } from "../../utils/api";
import { Button } from "@radix-ui/themes";
import type { PeerInfo } from "../../types";
import { PeerTableDialog } from "./peerTableDialog";

interface MemberCountProps {
  spaceId: string;
  connected: boolean;
}

export function MemberCount({ spaceId, connected }: MemberCountProps) {
  const { t } = useTranslation();
  const [peersList, setPeersList] = useState<PeerInfo[]>([]);
  const [showDialog, setShowDialog] = useState(false);

  useEffect(() => {
    if (!connected) {
      setPeersList([]);
      return;
    }
    let cancelled = false;
    const poll = async () => {
      try {
        const data = await getSpacePeers(spaceId);
        if (!cancelled) setPeersList(data);
      } catch (err) {
        console.log(err);
      }
    };
    poll();
    const timer = setInterval(poll, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [spaceId, connected]);

  if (!connected) return null;

  return (
    <>
      <Button
        onClick={() => setShowDialog(true)}
        variant="ghost"
        size="1"
        className="inline-flex items-center gap-1"
        title={t("space.viewOnlineMembers")}
      >
        <Users size={12} />
        <span>{t("space.memberCount", { count: peersList.length })}</span>
      </Button>
      <PeerTableDialog open={showDialog} openChange={setShowDialog} peerList={peersList} />
    </>
  );
}

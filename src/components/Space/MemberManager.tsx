import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSpaceStore } from "../../stores/spaceStore";
import { listMembers, removeMember } from "../../utils/api";
import type { Member } from "../../types";
import { Button, Dialog, ScrollArea, Text } from "@radix-ui/themes";
import { X, UserMinus } from "lucide-react";

interface MemberManagerProps {
  spaceId: string;
  callerId: string;
  onClose: () => void;
}

export function MemberManager({ spaceId, callerId, onClose }: MemberManagerProps) {
  const { t } = useTranslation();
  const [members, setMembers] = useState<Member[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { removeMember, loadSpaces } = useSpaceStore();

  const fetchMembers = async () => {
    try {
      setLoading(true);
      const data = await listMembers(spaceId);
      setMembers(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMembers();
  }, [spaceId]);

  const handleRemove = async (member: Member) => {
    try {
      await removeMember(spaceId, member.id, callerId);
      await fetchMembers();
      await loadSpaces();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl w-[540px] max-h-[80vh] flex flex-col shadow-xl">
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border)]">
          <h2 className="text-lg font-semibold">{t("space.memberManager")}</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>
        <div className="flex-1 p-6 overflow-y-auto">
          {loading ? (
            <p className="text-center py-8 text-[var(--color-text-secondary)]">{t("common.loading")}</p>
          ) : error ? (
            <p className="text-center py-8 text-[var(--color-danger)]">{error}</p>
          ) : members.length === 0 ? (
            <p className="text-center py-8 text-[var(--color-text-secondary)]">{t("space.noMembers")}</p>
          ) : (
            <div className="space-y-2">
              {members.map((m) => (
                <div
                  key={m.id}
                  className="flex items-center justify-between px-4 py-3 rounded-lg border border-[var(--color-border)]"
                >
                  <div>
                    <p className="font-medium">{m.nickname}</p>
                    {m.virtual_ip && (
                      <p className="text-xs text-[var(--color-text-secondary)] font-mono">
                        {m.virtual_ip}
                      </p>
                    )}
                    {m.is_online ? (
                      <p className="text-xs text-[var(--color-success)]">{t("space.online")}</p>
                    ) : (
                      <p className="text-xs text-[var(--color-text-secondary)]">{t("space.offline")}</p>
                    )}
                  </div>
                  <Button
                    onClick={() => handleRemove(m)}
                    variant="ghost"
                    color="red"
                    size="2"
                    title={t("space.removeMember")}
                  >
                    <UserMinus size={16} />
                  </Button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

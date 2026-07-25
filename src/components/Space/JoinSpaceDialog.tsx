import { useState } from "react";
import { useSpaceStore } from "../../stores/spaceStore";
import { X } from "lucide-react";
import { Button, TextField, Flex } from "@radix-ui/themes";

interface JoinSpaceDialogProps {
  onClose: () => void;
}

export function JoinSpaceDialog({ onClose }: JoinSpaceDialogProps) {
  const [networkName, setNetworkName] = useState("");
  const [networkSecret, setNetworkSecret] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const joinSpace = useSpaceStore((s) => s.joinSpace);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!networkName.trim() || !networkSecret.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await joinSpace(networkName.trim(), networkSecret.trim());
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handlePasteLink = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text.startsWith("homeTier://join?")) {
        const url = new URL(text);
        const name = url.searchParams.get("name");
        const secret = url.searchParams.get("secret");
        if (name && secret) {
          setNetworkName(name);
          setNetworkSecret(secret);
        }
      }
    } catch (err) {
      console.log(err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[var(--color-surface)] rounded-xl p-6 w-96 shadow-xl animate-fade-in">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">加入空间</h2>
          <Button onClick={onClose} variant="ghost" size="2">
            <X size={20} />
          </Button>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block mb-1 text-sm font-medium">网络名称 (Network Name)</label>
            <TextField.Root
              value={networkName}
              onChange={(e) => setNetworkName(e.target.value)}
              placeholder="输入网络名称"
              autoFocus
            />
          </div>
          <div>
            <label className="block mb-1 text-sm font-medium">网络密钥 (Network Secret)</label>
            <TextField.Root
              type="password"
              value={networkSecret}
              onChange={(e) => setNetworkSecret(e.target.value)}
              placeholder="输入网络密钥"
            />
          </div>
          <Button type="button" onClick={handlePasteLink} variant="ghost" color="blue" size="1">
            从剪贴板粘贴分享链接
          </Button>
          {error && (
            <p className="text-xs text-[var(--color-danger)]">{error}</p>
          )}
          <Flex justify="end" gap="2" pt="2">
            <Button type="button" onClick={onClose} variant="outline" size="2">
              取消
            </Button>
            <Button type="submit" disabled={loading || !networkName.trim() || !networkSecret.trim()} variant="solid" color="blue" size="2" loading={loading}>
              {loading ? "加入中..." : "加入"}
            </Button>
          </Flex>
        </form>
      </div>
    </div>
  );
}
import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { useFileStore } from "../../stores/fileStore";
import { formatFileSize, formatTimestamp } from "../../utils/format";
import * as api from "../../utils/api";
import { Button } from "@radix-ui/themes";
import { Download, Lock, FileText, ArrowLeft } from "lucide-react";
import { useNavigate } from "react-router-dom";

export function FileList() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { files, setFiles } = useFileStore();
  const [loading, setLoading] = useState(false);

  const spaceFiles = id ? files[id] || [] : [];

  useEffect(() => {
    if (id) {
      loadFiles();
    }
  }, [id]);

  const loadFiles = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const result = await api.listFiles(id);
      setFiles(id, result);
    } catch (e) {
      console.error("Load files failed:", e);
    } finally {
      setLoading(false);
    }
  };

  const handleFileSelect = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file || !id) return;
      try {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        await api.sendFile(id, (file as any).path || file.name);
        loadFiles();
      } catch (err) {
        console.error("Send file failed:", err);
      }
    };
    input.click();
  };

  return (
    <div className="flex flex-col flex-1">
      <div className="h-14 flex items-center gap-3 px-4 border-b border-[var(--color-border)] bg-[var(--color-surface)]">
        <Button
          onClick={() => navigate(`/space/${id}`)}
          variant="ghost"
          size="2"
        >
          <ArrowLeft size={20} />
        </Button>
        <span className="font-semibold">文件共享</span>
        <div className="flex-1" />
        <Button onClick={handleFileSelect} variant="solid" color="blue" size="2">
          发送文件
        </Button>
      </div>

      <div className="flex-1 p-4 overflow-y-auto">
        {loading ? (
          <div className="text-center py-8 text-[var(--color-text-secondary)]">加载中...</div>
        ) : spaceFiles.length === 0 ? (
          <div className="text-center py-20 text-[var(--color-text-secondary)]">
            <FileText size={48} className="mx-auto mb-3 opacity-50" />
            <p>暂无文件</p>
          </div>
        ) : (
          <div className="space-y-2">
            {spaceFiles.map((file) => (
              <div
                key={file.id}
                className="flex items-center gap-3 p-3 rounded-xl bg-[var(--color-surface)] border border-[var(--color-border)]"
              >
                <FileText size={24} className="text-[var(--color-primary)]" />
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">{file.file_name}</div>
                  <div className="text-xs text-[var(--color-text-secondary)]">
                    {formatFileSize(file.file_size)}
                    {file.is_compressed && " · 已压缩"}
                    {file.is_password_protected && (
                      <span className="ml-1 inline-flex items-center gap-0.5">
                        <Lock size={10} /> 加密
                      </span>
                    )}
                    <span className="ml-2">{formatTimestamp(file.created_at)}</span>
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="2"
                  title="下载"
                >
                  <Download size={18} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
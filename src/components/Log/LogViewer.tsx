import { useState, useEffect, useCallback } from "react";
import { getLogs, getSpaceLogs, clearLogs } from "../../utils/api";
import type { LogEntry } from "../../types";
import { RefreshCw, Trash2, Filter } from "lucide-react";
import { Button, Select, Checkbox, ScrollArea, Text, Flex, Badge, ButtonProps } from "@radix-ui/themes";

interface LogViewerProps {
  spaceId?: string;
}

const LEVEL_COLORS: Record<string, ButtonProps['color']> = {
  error: "red",
  warning: "yellow",
  info: "gray",
  debug: "teal",
};

export function LogViewer({ spaceId }: LogViewerProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  const fetchLogs = useCallback(async () => {
    try {
      const level = levelFilter === "all" ? undefined : levelFilter;
      const data = spaceId
        ? await getSpaceLogs(spaceId, level)
        : await getLogs(level);
      setLogs(data);
    } catch (e) {
      console.error("Failed to fetch logs:", e);
    }
  }, [spaceId, levelFilter]);

  useEffect(() => {
    fetchLogs();
    if (!autoRefresh) return;
    const timer = setInterval(fetchLogs, 2000);
    return () => clearInterval(timer);
  }, [fetchLogs, autoRefresh]);

  const handleClear = async () => {
    await clearLogs();
    setLogs([]);
  };

  const filtered = levelFilter === "all"
    ? logs
    : logs.filter((l) => l.level === levelFilter);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* 工具栏 */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>级别：</span>
        </div>
        <Select.Root value={levelFilter} onValueChange={(v) => setLevelFilter(v)}>
          <Select.Trigger className="text-xs" />
          <Select.Content>
            <Select.Item value="all">全部</Select.Item>
            <Select.Item value="error">Error</Select.Item>
            <Select.Item value="warning">Warning</Select.Item>
            <Select.Item value="info">Info</Select.Item>
            <Select.Item value="debug">Debug</Select.Item>
          </Select.Content>
        </Select.Root>

        <div className="flex-1" />

        <Flex gap="3" >
        <Text as="label" size="1" className="flex items-center gap-1 cursor-pointer">
          <Checkbox
            checked={autoRefresh}
            onCheckedChange={(c) => setAutoRefresh(c === true)}
          />
          自动刷新
        </Text>

        <Button
          onClick={fetchLogs}
          variant="ghost"
          size="2"
          title="刷新"
        >
          <RefreshCw size={16} />
        </Button>
        <Button
          onClick={handleClear}
          variant="ghost"
          color="red"
          size="2"
          title="清空日志"
        >
          <Trash2 size={16} />
        </Button>
        </Flex>
      </div>

      {/* 日志列表 */}
      <ScrollArea className="flex-1">
        <div className="h-full font-mono text-xs">
          {filtered.length === 0 ? (
            <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)] p-4">
              暂无日志
            </div>
          ) : (
            <div className="p-2 space-y-0.5">
              {filtered.map((entry, i) => (
                <div
                  key={i}
                  className="flex gap-2 px-2 py-1 rounded hover:bg-[var(--color-border)]/50"
                >
                  <span className="text-[var(--color-text-secondary)] whitespace-nowrap">
                    {entry.timestamp}
                  </span>
                  <Badge
                    className={`px-1 rounded font-bold`}
                    color={LEVEL_COLORS[entry.level]}
                  >
                    <span className="uppercase">{entry.level}</span>
                  </Badge>
                  <span className="text-[var(--color-text-secondary)] whitespace-nowrap">
                    [{entry.module}]
                  </span>
                  <span className="text-[var(--color-text)] break-all">
                    {entry.message}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </ScrollArea>

      {/* 底部统计 */}
      <div className="px-4 py-1.5 border-t border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-text-secondary)]">
        共 {filtered.length} 条日志
        {levelFilter !== "all" && ` (筛选: ${levelFilter})`}
      </div>
    </div>
  );
}
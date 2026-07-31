import { useState, useEffect, useCallback, useRef, useLayoutEffect, useMemo } from "react";
import { getLogs, getSpaceLogs, clearLogs } from "../../utils/api";
import type { LogEntry } from "../../types";
import { RefreshCw, Trash2, Filter } from "lucide-react";
import { Button, Select, Checkbox, Text, Flex, Badge, ButtonProps } from "@radix-ui/themes";
import { List, useDynamicRowHeight, type RowComponentProps } from "react-window";

interface LogViewerProps {
  spaceId?: string;
}

const LEVEL_COLORS: Record<string, ButtonProps['color']> = {
  error: "red",
  warning: "yellow",
  info: "gray",
  debug: "teal",
};

const DEFAULT_ROW_HEIGHT = 26;

interface LogRowProps {
  logs: LogEntry[];
}

const LogRow = ({ index, style, logs }: RowComponentProps<LogRowProps>) => {
  const entry = logs[index];

  if (!entry) return null;

  return (
    <div style={style} className="flex gap-2 px-2 py-0.5 items-start">
      <span className="text-[var(--color-text-secondary)] whitespace-nowrap leading-[26px]">
        {entry.timestamp}
      </span>
      <Badge className="px-1 rounded font-bold mt-0.5" color={LEVEL_COLORS[entry.level]}>
        <span className="uppercase">{entry.level}</span>
      </Badge>
      <span className="text-[var(--color-text-secondary)] whitespace-nowrap leading-[26px]">
        [{entry.module}]
      </span>
      <span className="text-[var(--color-text)] break-all leading-[26px]">
        {entry.message}
      </span>
    </div>
  );
};

function useSize<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return { ref, ...size };
}

export function LogViewer({ spaceId }: LogViewerProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const lastSeqRef = useRef(0);
  const fetchingRef = useRef(false);
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const { ref: containerRef, width, height } = useSize<HTMLDivElement>();
  const dynamicRowHeight = useDynamicRowHeight({ defaultRowHeight: DEFAULT_ROW_HEIGHT, key: "log-rows" });

  const fetchLogs = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    try {
      const level = levelFilter === "all" ? undefined : levelFilter;
      if (spaceId) {
        const data = await getSpaceLogs(spaceId, level);
        setLogs([...data].reverse());
      } else {
        const data = await getLogs(level, lastSeqRef.current || undefined);
        if (data.length > 0) {
          const maxSeq = Math.max(...data.map((l) => l.seq));
          lastSeqRef.current = Math.max(lastSeqRef.current, maxSeq);
          setLogs((prev) => {
            const sorted = [...data].reverse();
            return [...sorted, ...prev];
          });
        }
      }
    } catch (e) {
      console.error("Failed to fetch logs:", e);
    } finally {
      fetchingRef.current = false;
    }
  }, [spaceId, levelFilter]);

  useEffect(() => {
    setLogs([]);
    lastSeqRef.current = 0;
    fetchLogs();
    if (!autoRefresh) return;
    const timer = setInterval(fetchLogs, 2000);
    return () => clearInterval(timer);
  }, [fetchLogs, autoRefresh]);

  const handleClear = async () => {
    await clearLogs();
    setLogs([]);
    lastSeqRef.current = 0;
  };

  const isEasytier = (module: string) => module.startsWith("home_tier_lib::easytier");

  let filtered = logs;
  if (levelFilter !== "all") {
    filtered = filtered.filter((l) => l.level === levelFilter);
  }
  if (sourceFilter === "easytier") {
    filtered = filtered.filter((l) => isEasytier(l.module));
  } else if (sourceFilter === "system") {
    filtered = filtered.filter((l) => !isEasytier(l.module));
  }

  const rowKey = useCallback((index: number, data: LogRowProps) => {
    const entry = data.logs[index];
    return entry ? `${entry.seq}-${entry.module}-${index}` : String(index);
  }, []);

  const rowProps: LogRowProps = useMemo(
    () => ({ logs: filtered }),
    [filtered]
  );

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
            <Select.Item value="all">全部级别</Select.Item>
            <Select.Item value="error">Error</Select.Item>
            <Select.Item value="warning">Warning</Select.Item>
            <Select.Item value="info">Info</Select.Item>
            <Select.Item value="debug">Debug</Select.Item>
          </Select.Content>
        </Select.Root>

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>来源：</span>
        </div>
        <Select.Root value={sourceFilter} onValueChange={(v) => setSourceFilter(v)}>
          <Select.Trigger className="text-xs" />
          <Select.Content>
            <Select.Item value="all">全部</Select.Item>
            <Select.Item value="system">系统</Select.Item>
            <Select.Item value="easytier">EasyTier</Select.Item>
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
      <div ref={containerRef} className="flex-1 min-h-0">
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)] p-4">
            暂无日志
          </div>
        ) : (
          <List<LogRowProps>
            className="font-mono text-xs"
            rowCount={filtered.length}
            rowComponent={LogRow}
            rowProps={rowProps}
            rowHeight={dynamicRowHeight}
            rowKey={rowKey}
            overscanCount={8}
            style={{ width, height }}
          />
        )}
      </div>

      {/* 底部统计 */}
      <div className="px-4 py-1.5 border-t border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-text-secondary)]">
        共 {filtered.length} 条日志
        {sourceFilter !== "all" && ` (来源: ${sourceFilter === "easytier" ? "EasyTier" : "系统"})`}
        {levelFilter !== "all" && ` (级别: ${levelFilter})`}
      </div>
    </div>
  );
}

import { useState, useEffect, useCallback, useRef, useLayoutEffect, useMemo } from "react";
import { queryLogs, getLogModules, clearLogsFiltered } from "../../utils/api";
import type { LogEntry } from "../../types";
import { RefreshCw, Trash2, Filter, Search } from "lucide-react";
import { Button, Select, Checkbox, Text, Flex, Badge, Dialog, ButtonProps } from "@radix-ui/themes";
import { List, useDynamicRowHeight, type RowComponentProps } from "react-window";

interface LogViewerProps {
  spaceId?: string;
}

const CATEGORIES = [
  "system", "network", "webrtc", "data", "proxy", "daemon", "space", "server",
] as const;

const LEVEL_COLORS: Record<string, ButtonProps["color"]> = {
  error: "red",
  warning: "yellow",
  info: "gray",
  debug: "teal",
};

const DEFAULT_ROW_HEIGHT = 26;

const CATEGORY_LABELS: Record<string, string> = {
  system: "系统",
  network: "网络",
  webrtc: "语音/屏幕",
  data: "数据传输",
  proxy: "代理",
  daemon: "守护进程",
  space: "空间",
  server: "服务端",
};

interface LogRowProps {
  logs: LogEntry[];
}

const LogRow = ({ index, style, logs }: RowComponentProps<LogRowProps>) => {
  const entry = logs[index];
  if (!entry) return null;

  const cls = `leading-[${DEFAULT_ROW_HEIGHT}px]`;
  return (
    <div style={style} className="flex gap-2 px-2 py-0.5 items-start border-b border-[var(--color-border)] hover:bg-[var(--color-accent)]/5">
      <span className={`text-[var(--color-text-secondary)] whitespace-nowrap ${cls} w-28 shrink-0`}>
        {entry.timestamp.slice(11, 24)}
      </span>
      <Badge className="px-0.5 rounded mt-0.5 w-14 shrink-0" color={LEVEL_COLORS[entry.level]}>
        <span className="w-full text-center uppercase text-[10px]">{entry.level}</span>
      </Badge>
      <Badge className="px-0.5 rounded mt-0.5 w-16 shrink-0" color="violet">
        <span className="w-full text-center text-[10px] capitalize">{entry.category}</span>
      </Badge>
      <span className={`text-[var(--color-text-secondary)] whitespace-nowrap ${cls} w-20 shrink-0`}>
        {(entry.module || "").replace("home_tier_lib::", "").replace("easytier::", "et::")}
      </span>
      <span className={`text-[var(--color-text)] break-all ${cls} flex-1 min-w-0`}>
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

function capitalize(s: string) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function LogViewer({ spaceId }: LogViewerProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const lastSeqRef = useRef(0);
  const fetchingRef = useRef(false);
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [categoryFilter, setCategoryFilter] = useState<string[]>([]);
  const [moduleFilter, setModuleFilter] = useState<string[]>([]);
  const [keyword, setKeyword] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [showClearDialog, setShowClearDialog] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [modules, setModules] = useState<string[]>([]);
  const { ref: containerRef, width, height } = useSize<HTMLDivElement>();
  const dynamicRowHeight = useDynamicRowHeight({ defaultRowHeight: DEFAULT_ROW_HEIGHT, key: "log-rows" });

  const fetchModules = useCallback(async () => {
    try {
      const mods = await getLogModules();
      setModules(mods);
    } catch (e) {
      console.error("Failed to fetch modules:", e);
    }
  }, []);

  const fetchLogs = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    try {
      const filter = {
        level: levelFilter === "all" ? undefined : levelFilter,
        space_id: spaceId || undefined,
        module: moduleFilter.length ? moduleFilter.join(",") : undefined,
        category: categoryFilter.length ? categoryFilter.join(",") : undefined,
        keyword: keyword || undefined,
        since_seq: lastSeqRef.current || undefined,
        limit: 50000,
      };
      const data = await queryLogs(filter);
      if (data.length > 0) {
        const maxSeq = Math.max(...data.map((l) => l.seq));
        lastSeqRef.current = Math.max(lastSeqRef.current, maxSeq);
        setLogs((prev) => [...[...data].reverse(), ...prev]);
      }
    } catch (e) {
      console.error("Failed to fetch logs:", e);
    } finally {
      fetchingRef.current = false;
    }
  }, [spaceId, levelFilter, categoryFilter, moduleFilter, keyword]);

  const handleClear = useCallback(async () => {
    if (clearing) return;
    setClearing(true);
    setShowClearDialog(false);
    try {
      await clearLogsFiltered({
        level: levelFilter === "all" ? undefined : levelFilter,
        space_id: spaceId || undefined,
        module: moduleFilter.length ? moduleFilter.join(",") : undefined,
        category: categoryFilter.length ? categoryFilter.join(",") : undefined,
        keyword: keyword || undefined,
      });
      setLogs([]);
      lastSeqRef.current = 0;
    } catch (e) {
      console.error("Failed to clear logs:", e);
    } finally {
      setClearing(false);
    }
  }, [clearing, spaceId, levelFilter, categoryFilter, moduleFilter, keyword]);

  useEffect(() => {
    fetchModules();
  }, [fetchModules]);

  useEffect(() => {
    setLogs([]);
    lastSeqRef.current = 0;
    fetchLogs();
    if (!autoRefresh) return;
    const timer = setInterval(fetchLogs, 2000);
    return () => clearInterval(timer);
  }, [fetchLogs, autoRefresh, spaceId, levelFilter, categoryFilter, moduleFilter, keyword]);

  const filtered = useMemo(() => {
    let result = logs;
    if (levelFilter !== "all") {
      result = result.filter((l) => l.level === levelFilter);
    }
    if (categoryFilter.length > 0) {
      result = result.filter((l) => categoryFilter.includes(l.category));
    }
    if (moduleFilter.length > 0) {
      result = result.filter((l) => moduleFilter.includes(l.module));
    }
    if (keyword) {
      const kw = keyword.toLowerCase();
      result = result.filter(
        (l) =>
          l.message.toLowerCase().includes(kw) ||
          l.target.toLowerCase().includes(kw) ||
          l.module.toLowerCase().includes(kw)
      );
    }
    return result;
  }, [logs, levelFilter, categoryFilter, moduleFilter, keyword]);

  const rowKey = useCallback((index: number, data: { logs: LogEntry[] }) => {
    const entry = data.logs[index];
    return entry ? `${entry.seq}-${entry.module}-${index}` : String(index);
  }, []);

  const rowProps = useMemo(() => ({ logs: filtered }), [filtered]);

  const totalCount = filtered.length;
  const displayCount = Math.min(totalCount, 5000);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>级别：</span>
        </div>
        <Select.Root size="1" value={levelFilter} onValueChange={(v) => setLevelFilter(v)}>
          <Select.Trigger className="text-xs w-28" />
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
          <span>分类：</span>
        </div>
        <Select.Root
          size="1"
          value={categoryFilter.length ? categoryFilter.join(",") : "all"}
          onValueChange={(v) => setCategoryFilter(v === "all" ? [] : v.split(","))}
        >
          <Select.Trigger className="text-xs w-32" />
          <Select.Content>
            <Select.Item value="all">全部分类</Select.Item>
            {CATEGORIES.map((cat) => (
              <Select.Item
                key={cat}
                value={categoryFilter.filter((c) => c !== cat).concat(cat).join(",") || "all"}
              >
                <Flex align="center" gap="2">
                  <Checkbox checked={categoryFilter.includes(cat)} onCheckedChange={() => {}} />
                  <span>{CATEGORY_LABELS[cat] || capitalize(cat)}</span>
                </Flex>
              </Select.Item>
            ))}
          </Select.Content>
        </Select.Root>

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>模块：</span>
        </div>
        <Select.Root
          size="1"
          value={moduleFilter.length ? moduleFilter.join(",") : "all"}
          onValueChange={(v) => setModuleFilter(v === "all" ? [] : v.split(","))}
        >
          <Select.Trigger className="text-xs w-32" />
          <Select.Content>
            <Select.Item value="all">全部模块</Select.Item>
            {modules.map((mod) => (
              <Select.Item
                key={mod}
                value={moduleFilter.filter((m) => m !== mod).concat(mod).join(",") || "all"}
              >
                <Flex align="center" gap="2">
                  <Checkbox checked={moduleFilter.includes(mod)} onCheckedChange={() => {}} />
                  <span className="truncate max-w-[160px]">{mod.replace("home_tier_lib::", "").replace("easytier::", "et::")}</span>
                </Flex>
              </Select.Item>
            ))}
          </Select.Content>
        </Select.Root>

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Search size={14} />
          <span>搜索：</span>
        </div>
        <div className="relative flex-1 min-w-[180px] max-w-[320px]">
          <input
            type="text"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="搜索消息/模块/目标..."
            className="w-full text-xs px-2 py-1 bg-[var(--color-background)] border border-[var(--color-border)] rounded"
          />
        </div>

        <div className="flex-1" />

        <Flex gap="2">
          <Text as="label" size="1" className="flex items-center gap-1 cursor-pointer">
            <Checkbox checked={autoRefresh} onCheckedChange={(c) => setAutoRefresh(c === true)} />
            自动刷新
          </Text>

          <Button onClick={fetchLogs} variant="ghost" size="2" title="刷新">
            <RefreshCw size={16} />
          </Button>

          <Dialog.Root open={showClearDialog} onOpenChange={setShowClearDialog}>
            <Button variant="ghost" size="2" color="red" title="清空日志" onClick={() => setShowClearDialog(true)}>
              <Trash2 size={16} />
            </Button>
            <Dialog.Content className="max-w-md">
              <Dialog.Title>确认清空日志</Dialog.Title>
              <Dialog.Description>
                此操作将清空当前筛选条件下的所有日志（当前显示 {filtered.length} 条）。
                {filtered.length === totalCount ? "" : `（共 ${totalCount} 条，当前筛选显示 ${filtered.length} 条）`}
                操作不可撤销，仅删除已存日志，后续新日志仍会正常记录。
              </Dialog.Description>
              <Flex gap="2" justify="end" style={{ marginTop: 16 }}>
                <Button variant="ghost" onClick={() => setShowClearDialog(false)}>取消</Button>
                <Button onClick={handleClear} color="red" disabled={clearing}>
                  {clearing ? "清空中..." : "确认清空"}
                </Button>
              </Flex>
            </Dialog.Content>
          </Dialog.Root>
        </Flex>
      </div>

      <div ref={containerRef} className="flex-1 min-h-0">
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)] p-4">
            暂无日志
          </div>
        ) : (
          <List<{ logs: LogEntry[] }>
            className="font-mono text-xs"
            rowCount={displayCount}
            rowComponent={LogRow}
            rowProps={rowProps}
            rowHeight={dynamicRowHeight}
            rowKey={rowKey}
            overscanCount={8}
            style={{ width, height }}
          />
        )}
      </div>

      <div className="px-4 py-1.5 border-t border-[var(--color-border)] bg-[var(--color-surface)] text-xs text-[var(--color-text-secondary)] flex flex-wrap items-center gap-2">
        <span>共 {totalCount} 条日志 {displayCount < totalCount && `（显示 ${displayCount}）`}</span>
        {categoryFilter.length > 0 && (
          <span className="px-1.5 py-0.5 bg-[var(--color-accent)]/20 rounded text-[10px]">
            {categoryFilter.map((c) => CATEGORY_LABELS[c] || c).join(", ")}
          </span>
        )}
        {moduleFilter.length > 0 && (
          <span className="px-1.5 py-0.5 bg-[var(--color-accent)]/20 rounded text-[10px]">
            {moduleFilter.slice(0, 3).join(", ")}
            {moduleFilter.length > 3 ? "..." : ""}
          </span>
        )}
        {keyword && (
          <span className="px-1.5 py-0.5 bg-[var(--color-warning)]/20 rounded text-[10px]">
            🔍 "{keyword}"
          </span>
        )}
        <span className="ml-auto text-[var(--color-text-muted)]">显示 {displayCount}/{totalCount}</span>
      </div>
    </div>
  );
}

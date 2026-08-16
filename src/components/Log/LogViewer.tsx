import { useState, useEffect, useCallback, useRef, useLayoutEffect, useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { queryLogs, queryDaemonLogs, getLogModules, clearLogsFiltered, exportLogs, isTauri } from "../../utils/api";
import type { LogEntry } from "../../types";
import { RefreshCw, Trash2, Filter, Search, Download, Copy, Clock } from "lucide-react";
import { Button, Select, Checkbox, Text, Flex, Badge, Dialog, DropdownMenu, ButtonProps } from "@radix-ui/themes";
import { List, useDynamicRowHeight, type RowComponentProps } from "react-window";
import { toastSuccess, toastError } from "../../utils/toast";

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

const CATEGORY_I18N_KEYS: Record<string, string> = {
  system: "log.categorySystem",
  network: "log.categoryNetwork",
  webrtc: "log.categoryVoice",
  data: "log.categoryDataTransfer",
  proxy: "log.categoryProxy",
  daemon: "log.categoryDaemon",
  space: "log.categorySpace",
  server: "log.categoryServer",
};

interface LogRowProps {
  logs: LogEntry[];
  onRowClick?: (entry: LogEntry) => void;
  keyword?: string;
}

function Highlight({ text, keyword }: { text: string; keyword?: string }) {
  if (!keyword) return <>{text}</>;
  const kw = keyword.toLowerCase();
  const lower = text.toLowerCase();
  const parts: ReactNode[] = [];
  let i = 0;
  while (true) {
    const idx = lower.indexOf(kw, i);
    if (idx === -1) {
      parts.push(text.slice(i));
      break;
    }
    if (idx > i) parts.push(text.slice(i, idx));
    parts.push(
      <mark key={idx} className="bg-[var(--color-warning)]/40 rounded-[2px]">
        {text.slice(idx, idx + kw.length)}
      </mark>
    );
    i = idx + kw.length;
  }
  return <>{parts}</>;
}

const LogRow = ({ index, style, logs, onRowClick, keyword }: RowComponentProps<LogRowProps>) => {
  const entry = logs[index];
  if (!entry) return null;

  const cls = `leading-[${DEFAULT_ROW_HEIGHT}px]`;
  return (
    <div
      style={style}
      onClick={() => onRowClick?.(entry)}
      className="flex gap-2 px-2 py-0.5 items-start border-b border-[var(--color-border)] hover:bg-[var(--color-accent)]/5 cursor-pointer"
    >
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
        <Highlight text={entry.message} keyword={keyword} />
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

type TimeRange = "all" | "1h" | "6h" | "24h" | "custom";

function computeTimeFilter(range: TimeRange, from?: string, to?: string) {
  if (range === "custom") {
    return {
      before_ts: to ? new Date(to).toISOString() : undefined,
      after_ts: from ? new Date(from).toISOString() : undefined,
    };
  }
  if (range === "all") {
    return { before_ts: undefined, after_ts: undefined };
  }
  const hours = range === "1h" ? 1 : range === "6h" ? 6 : 24;
  return {
    before_ts: undefined,
    after_ts: new Date(Date.now() - hours * 3600_000).toISOString(),
  };
}

export function LogViewer({ spaceId }: LogViewerProps) {
  const { t } = useTranslation();
  const [source, setSource] = useState<"gui" | "daemon">("gui");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const lastSeqRef = useRef(0);
  const fetchingRef = useRef(false);
  const [levelFilter, setLevelFilter] = useState<string>("all");
  const [categoryFilter, setCategoryFilter] = useState<string[]>([]);
  const [moduleFilter, setModuleFilter] = useState<string[]>([]);
  const [keyword, setKeyword] = useState("");
  const [timeRange, setTimeRange] = useState<TimeRange>("all");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [showClearDialog, setShowClearDialog] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [detailEntry, setDetailEntry] = useState<LogEntry | null>(null);
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
      if (source === "daemon") {
        const data = await queryDaemonLogs({
          level: levelFilter === "all" ? undefined : levelFilter,
          space_id: spaceId || undefined,
          category: categoryFilter.length ? categoryFilter.join(",") : undefined,
          keyword: keyword || undefined,
          limit: 50000,
        });
        setLogs(data.length ? [...data].reverse() : []);
        return;
      }
      const { before_ts, after_ts } = computeTimeFilter(timeRange, customFrom, customTo);
      const filter = {
        level: levelFilter === "all" ? undefined : levelFilter,
        space_id: spaceId || undefined,
        module: moduleFilter.length ? moduleFilter.join(",") : undefined,
        category: categoryFilter.length ? categoryFilter.join(",") : undefined,
        keyword: keyword || undefined,
        before_ts,
        after_ts,
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
  }, [source, spaceId, levelFilter, categoryFilter, moduleFilter, keyword, timeRange, customFrom, customTo]);

  const handleExport = useCallback(
    async (format: "txt" | "json") => {
      if (exporting) return;
      setExporting(true);
      try {
        const { before_ts, after_ts } = computeTimeFilter(timeRange, customFrom, customTo);
        const path = await exportLogs({
          level: levelFilter === "all" ? undefined : levelFilter,
          space_id: spaceId || undefined,
          module: moduleFilter.length ? moduleFilter.join(",") : undefined,
          category: categoryFilter.length ? categoryFilter.join(",") : undefined,
          keyword: keyword || undefined,
          before_ts,
          after_ts,
          format,
        });
        setExportPath(path);
      } catch (e) {
        toastError(String(e));
      } finally {
        setExporting(false);
      }
    },
    [exporting, spaceId, levelFilter, categoryFilter, moduleFilter, keyword, timeRange, customFrom, customTo]
  );

  const copyText = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toastSuccess(t("log.copiedToClipboard"));
    } catch (e) {
      toastError(t("log.copyFailed"));
    }
  }, []);

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
  }, [fetchLogs, autoRefresh, source]);

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

  const rowProps = useMemo(() => ({ logs: filtered, onRowClick: setDetailEntry, keyword }), [filtered, keyword]);

  const totalCount = filtered.length;
  const displayCount = Math.min(totalCount, 5000);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        {isTauri() && (
          <Select.Root size="1" value={source} onValueChange={(v) => setSource(v as "gui" | "daemon")}>
            <Select.Trigger className="text-xs w-28" />
            <Select.Content>
              <Select.Item value="gui">{t("log.localLogs")}</Select.Item>
              <Select.Item value="daemon">{t("log.daemonLogs")}</Select.Item>
            </Select.Content>
          </Select.Root>
        )}

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>{t("log.levelLabel")}</span>
        </div>
        <Select.Root size="1" value={levelFilter} onValueChange={(v) => setLevelFilter(v)}>
          <Select.Trigger className="text-xs w-28" />
          <Select.Content>
            <Select.Item value="all">{t("log.allLevels")}</Select.Item>
            <Select.Item value="error">Error</Select.Item>
            <Select.Item value="warning">Warning</Select.Item>
            <Select.Item value="info">Info</Select.Item>
            <Select.Item value="debug">Debug</Select.Item>
          </Select.Content>
        </Select.Root>

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Filter size={14} />
          <span>{t("log.categoryLabel")}</span>
        </div>
        <Select.Root
          size="1"
          value={categoryFilter.length ? categoryFilter.join(",") : "all"}
          onValueChange={(v) => setCategoryFilter(v === "all" ? [] : v.split(","))}
        >
          <Select.Trigger className="w-32 text-xs" />
          <Select.Content>
            <Select.Item value="all">{t("log.allCategories")}</Select.Item>
            {CATEGORIES.map((cat) => (
              <Select.Item
                key={cat}
                value={categoryFilter.filter((c) => c !== cat).concat(cat).join(",") || "all"}
              >
                <Flex align="center" gap="2">
                  <Checkbox checked={categoryFilter.includes(cat)} onCheckedChange={() => {}} />
                  <span>{t(CATEGORY_I18N_KEYS[cat] || capitalize(cat))}</span>
                </Flex>
              </Select.Item>
            ))}
          </Select.Content>
        </Select.Root>

        {source === "gui" && (
          <>
            <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
              <Filter size={14} />
          <span>{t("log.moduleLabel")}</span>
        </div>
        <Select.Root
          size="1"
          value={moduleFilter.length ? moduleFilter.join(",") : "all"}
          onValueChange={(v) => setModuleFilter(v === "all" ? [] : v.split(","))}
        >
          <Select.Trigger className="w-32 text-xs" />
          <Select.Content>
            <Select.Item value="all">{t("log.allModules")}</Select.Item>
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
          </>
        )}

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Search size={14} />
          <span>{t("log.searchLabel")}</span>
        </div>
        <div className="relative flex-1 min-w-[180px] max-w-[320px]">
          <input
            type="text"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder={t("log.searchPlaceholder")}
            className="w-full text-xs px-2 py-1 bg-[var(--color-background)] border border-[var(--color-border)] rounded"
          />
        </div>

        <div className="flex items-center gap-1 text-xs text-[var(--color-text-secondary)]">
          <Clock size={14} />
          <span>{t("log.timeRange")}</span>
        </div>
        <Select.Root size="1" value={timeRange} onValueChange={(v) => setTimeRange(v as TimeRange)}>
          <Select.Trigger className="text-xs w-28" />
          <Select.Content>
            <Select.Item value="all">{t("log.allTime")}</Select.Item>
            <Select.Item value="1h">{t("log.last1h")}</Select.Item>
            <Select.Item value="6h">{t("log.last6h")}</Select.Item>
            <Select.Item value="24h">{t("log.last24h")}</Select.Item>
            <Select.Item value="custom">{t("log.custom")}</Select.Item>
          </Select.Content>
        </Select.Root>
        {timeRange === "custom" && (
          <Flex gap="1" align="center" className="text-xs text-[var(--color-text-secondary)]">
            <input
              type="datetime-local"
              value={customFrom}
              onChange={(e) => setCustomFrom(e.target.value)}
              title={t("log.startTime")}
              className="text-xs px-1.5 py-1 bg-[var(--color-background)] border border-[var(--color-border)] rounded"
            />
            <span>—</span>
            <input
              type="datetime-local"
              value={customTo}
              onChange={(e) => setCustomTo(e.target.value)}
              title={t("log.endTime")}
              className="text-xs px-1.5 py-1 bg-[var(--color-background)] border border-[var(--color-border)] rounded"
            />
          </Flex>
        )}

        <div className="flex-1" />

        <Flex gap="2">
          <Text as="label" size="1" className="flex items-center gap-1 cursor-pointer">
            <Checkbox checked={autoRefresh} onCheckedChange={(c) => setAutoRefresh(c === true)} />
            {t("log.autoRefresh")}
          </Text>

          {source === "gui" && (
            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                <Button variant="ghost" size="2" disabled={exporting} title={t("log.exportLogs")}>
                  <Download size={16} />
                </Button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Content>
                <DropdownMenu.Item onClick={() => handleExport("txt")} disabled={exporting}>
                  {exporting ? t("log.exporting") : t("log.exportTxt")}
                </DropdownMenu.Item>
                <DropdownMenu.Item onClick={() => handleExport("json")} disabled={exporting}>
                  {exporting ? t("log.exporting") : t("log.exportJson")}
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          )}

          <Button onClick={fetchLogs} variant="ghost" size="2" title={t("log.refresh")}>
            <RefreshCw size={16} />
          </Button>

          {source === "gui" && (
            <Dialog.Root open={showClearDialog} onOpenChange={setShowClearDialog}>
              <Button variant="ghost" size="2" color="red" title={t("log.clearLogs")} onClick={() => setShowClearDialog(true)}>
                <Trash2 size={16} />
              </Button>
              <Dialog.Content className="max-w-md">
                <Dialog.Title>{t("log.confirmClear")}</Dialog.Title>
                <Dialog.Description>
                  {t("log.clearDesc", { count: filtered.length })}
                  {filtered.length === totalCount ? "" : ` ${t("log.clearCount", { total: totalCount, count: filtered.length })}`}
                  {t("log.clearWarning")}
                </Dialog.Description>
                <Flex gap="2" align="center" justify="end" style={{ marginTop: 16 }}>
                  <Button variant="ghost" onClick={() => setShowClearDialog(false)}>{t("common.cancel")}</Button>
                  <Button onClick={handleClear} size="1" color="red" disabled={clearing}>
                    {clearing ? t("log.clearing") : t("log.confirmClearBtn")}
                  </Button>
                </Flex>
              </Dialog.Content>
            </Dialog.Root>
          )}
        </Flex>
      </div>

      <Dialog.Root open={!!detailEntry} onOpenChange={(o) => !o && setDetailEntry(null)}>
        <Dialog.Content className="max-w-lg">
          <Dialog.Title>{t("log.detail")}</Dialog.Title>
          {detailEntry && (
            <>
              <Flex gap="2" align="center" wrap="wrap" className="mb-2 text-xs text-[var(--color-text-secondary)]">
                <Badge color={LEVEL_COLORS[detailEntry.level]}>
                  {detailEntry.level.toUpperCase()}
                </Badge>
                <Badge color="violet">{t(CATEGORY_I18N_KEYS[detailEntry.category] || detailEntry.category)}</Badge>
                <span className="font-mono">{detailEntry.module}</span>
              </Flex>
              <Flex direction="column" gap="1" className="mb-3 text-xs text-[var(--color-text-secondary)] font-mono">
                <span>{t("log.detailTime")} {detailEntry.timestamp}</span>
                {detailEntry.space_id && <span>{t("log.detailSpace")} {detailEntry.space_id}</span>}
                {detailEntry.trace_id && <span>{t("log.detailTrace")} {detailEntry.trace_id}</span>}
                <span>{t("log.detailSeq")} {detailEntry.seq}</span>
              </Flex>
              <div className="max-h-[300px] overflow-y-auto text-xs text-[var(--color-text)] whitespace-pre-wrap break-all p-2 bg-[var(--color-background)] border border-[var(--color-border)] rounded">
                {detailEntry.message}
              </div>
              <Flex gap="2" align="center" justify="end" style={{ marginTop: 16 }}>
                <Button variant="ghost" size="1" onClick={() => copyText(detailEntry.message)}>
                  <Copy size={14} />
                  {t("log.copyMessage")}
                </Button>
                <Button size="1" onClick={() => setDetailEntry(null)}>{t("common.close")}</Button>
              </Flex>
            </>
          )}
        </Dialog.Content>
      </Dialog.Root>

      <Dialog.Root open={!!exportPath} onOpenChange={(o) => !o && setExportPath(null)}>
        <Dialog.Content className="max-w-lg">
          <Dialog.Title>{t("log.exportSuccess")}</Dialog.Title>
          <Dialog.Description>{t("log.exportPath")}</Dialog.Description>
          {exportPath && (
            <Flex gap="2" align="center" className="mt-2">
              <code className="flex-1 min-w-0 text-xs font-mono bg-[var(--color-background)] border border-[var(--color-border)] rounded px-2 py-1 break-all">
                {exportPath}
              </code>
              <Button variant="ghost" size="1" onClick={() => copyText(exportPath)} title={t("log.copyPath")}>
                <Copy size={14} />
              </Button>
            </Flex>
          )}
          <Flex justify="end" style={{ marginTop: 16 }}>
            <Button onClick={() => setExportPath(null)}>{t("common.confirm")}</Button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>

      <div ref={containerRef} className="flex-1 min-h-0">
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)] p-4">
            {t("log.empty")}
          </div>
        ) : (
          <List<{ logs: LogEntry[]; onRowClick?: (e: LogEntry) => void; keyword?: string }>
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
        <span>{t("log.total", { count: totalCount })} {displayCount < totalCount && ` ${t("log.showing", { count: displayCount })}`}</span>
        {categoryFilter.length > 0 && (
          <span className="px-1.5 py-0.5 bg-[var(--color-accent)]/20 rounded text-[10px]">
            {categoryFilter.map((c) => t(CATEGORY_I18N_KEYS[c] || c)).join(", ")}
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
        <span className="flex items-center gap-1.5">
          {(["error", "warning", "info", "debug"] as const).map((lv) => {
            const n = filtered.filter((l) => l.level === lv).length;
            if (n === 0) return null;
            return (
              <span
                key={lv}
                onClick={() => setLevelFilter(levelFilter === lv ? "all" : lv)}
                className="px-1.5 py-0.5 rounded text-[10px] cursor-pointer hover:opacity-70"
                style={{ color: `var(--${LEVEL_COLORS[lv]}-10)` }}
              >
                {lv} {n}
              </span>
            );
          })}
        </span>
        <span className="ml-auto text-[var(--color-text-muted)]">{t("log.showingOf", { count: displayCount, total: totalCount })}</span>
      </div>
    </div>
  );
}

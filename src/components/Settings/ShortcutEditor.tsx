import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Kbd, Flex, Text } from "@radix-ui/themes";
import { useSettingsStore } from "../../stores/settingsStore";

interface ShortcutEditorProps {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  description?: string;
}

export function ShortcutEditor({ value, onChange, placeholder = "未设置", description }: ShortcutEditorProps) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const containerRef = useRef<HTMLDivElement>(null);
  const initialValueRef = useRef(value);

  // 编辑态标志：编辑中屏蔽全局/页面内快捷键触发
  useEffect(() => {
    useSettingsStore.getState().setShortcutEditing(editing);
    return () => useSettingsStore.getState().setShortcutEditing(false);
  }, [editing]);

  // 监听 keydown（编辑态时）
  useEffect(() => {
    if (!editing) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();

      const key = e.key;
      const code = e.code;

      // Escape 取消
      if (key === "Escape") {
        setDraft(initialValueRef.current);
        setEditing(false);
        return;
      }

      // Enter 确认
      if (key === "Enter") {
        onChange(draft);
        setEditing(false);
        return;
      }

      // 仅修饰键单按时忽略
      if (["Control", "Shift", "Alt", "Meta"].includes(key)) {
        return;
      }

      // 收集修饰键（顺序：Ctrl < Alt < Shift < Meta）
      const mods: string[] = [];
      if (e.ctrlKey) mods.push("Ctrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      if (e.metaKey) mods.push("Meta");

      // 解析主键
      let mainKey = "";
      if (code.startsWith("Key")) {
        mainKey = code.slice(3); // KeyA -> A
      } else if (code.startsWith("Digit")) {
        mainKey = code.slice(5); // Digit1 -> 1
      } else if (code.startsWith("F") && /^F\d+$/.test(code)) {
        mainKey = code; // F1, F12 等
      } else {
        // 其他特殊键用 code 形式（如 ArrowUp, Tab, Space, Enter, Escape, Slash, Period 等）
        const specialMap: Record<string, string> = {
          Space: "Space",
          Tab: "Tab",
          Enter: "Enter",
          Escape: "Escape",
          Backspace: "Backspace",
          ArrowUp: "ArrowUp",
          ArrowDown: "ArrowDown",
          ArrowLeft: "ArrowLeft",
          ArrowRight: "ArrowRight",
          Slash: "/",
          Backslash: "\\",
          Period: ".",
          Comma: ",",
          Semicolon: ";",
          Quote: "'",
          BracketLeft: "[",
          BracketRight: "]",
          Minus: "-",
          Equal: "=",
          Grave: "`",
        };
        mainKey = specialMap[code] || code;
      }

      if (mainKey) {
        const shortcut = [...mods, mainKey].join("+");
        setDraft(shortcut);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [editing, draft, onChange]);

  // blur 保存
  const handleBlur = useCallback(() => {
    if (editing) {
      onChange(draft);
      setEditing(false);
    }
  }, [editing, draft, onChange]);

  // 格式化展示
  const renderKbdChain = (str: string) => {
    if (!str) return <Text size="2" className="text-[var(--color-text-tertiary)]">{placeholder}</Text>;
    return str.split("+").map((token, idx) => (
      <Flex key={idx} align="center" gap="1">
        <Kbd size="1">{token}</Kbd>
        {idx < str.split("+").length - 1 && <Text size="2" className="text-[var(--color-text-tertiary)]">+</Text>}
      </Flex>
    ));
  };

  return (
    <>
      <Flex
        ref={containerRef}
        align="center"
        gap="2"
        className="w-full max-w-xs cursor-pointer"
        onClick={() => {
          if (!editing) {
            initialValueRef.current = value;
            setDraft(value);
            setEditing(true);
          }
        }}
        onBlur={handleBlur}
        tabIndex={0}
        role="button"
        aria-label={t("settings.shortcutsHelp")}
      >
        <Flex align="center" gap="1" className="flex-1 min-w-0">
          {renderKbdChain(editing ? draft : value)}
          {editing && (
            <>
              <Kbd size="1" className="text-[var(--color-text-tertiary)] opacity-40">
                {t("settings.shortcutEditHint")}
              </Kbd>
            </>
          )}
        </Flex>
      </Flex>
      {description && (
        <Text size="1" color="gray" className="max-w-xs">
          {description}
        </Text>
      )}
    </>
  );
}
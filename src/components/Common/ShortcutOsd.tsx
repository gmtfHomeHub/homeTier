import { useTranslation } from "react-i18next";
import { Mic, MicOff, Volume2, VolumeX } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { useShortcutOsdStore } from "../../stores/shortcutOsdStore";

export function ShortcutOsd() {
  const { t } = useTranslation();
  const { visible, kind, muted, notJoined, tick } = useShortcutOsdStore();

  const IconComp =
    kind === "mic" ? (muted ? MicOff : Mic) : muted ? VolumeX : Volume2;

  const label = notJoined
    ? t("shortcuts.notJoined")
    : muted
      ? kind === "mic"
        ? t("shortcuts.micMuted")
        : t("shortcuts.speakerMuted")
      : kind === "mic"
        ? t("shortcuts.micUnmuted")
        : t("shortcuts.speakerUnmuted");

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          key={tick}
          initial={{ opacity: 0, y: -16 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -16 }}
          transition={{ duration: 0.18 }}
          className="fixed top-16 left-1/2 -translate-x-1/2 z-[999] pointer-events-none flex items-center gap-3 rounded-full bg-black/75 text-white px-5 py-2.5 shadow-xl backdrop-blur-sm"
        >
          <span className="w-6 h-6 flex items-center justify-center">
            <IconComp size={20} />
          </span>
          <span className="text-sm font-medium whitespace-nowrap">{label}</span>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// src/components/Common/Icon.tsx — 自适应 lucide 图标封装
//
// 包装项目已用的 lucide 图标，按全局自适应档位 L∈{1,2,3}（紧凑/标准/宽松）
// 对 size 做乘法缩放：effective = round(size × [0.85,1.0,1.2][L-1])。
//   - 未声明 size（lucide 默认 24）：按 24 乘。
//   - 其余 props（color/strokeWidth/absoluteStrokeWidth/className/children/ref/任意 SVG prop）透传。
// 调用方仅改 import 来源：from "." → from "../Common"（Common 内部用 "."），JSX 零改动。
// lucide-react sideEffects:false → import * as Lucide + 静态 Lucide.X 仅打包已用图标，无体积膨胀。

import { forwardRef } from "react";
import * as Lucide from "lucide-react";
import type { LucideProps, LucideIcon } from "lucide-react";
import { useSettingsStore } from "../../stores/settingsStore";

// 与 App.tsx ADAPTIVE_SPACE_MULT 同值（密度一致）；独立常量便于将来单独调图标缩放
const ADAPTIVE_ICON_MULT = [0.85, 1.0, 1.2];

function adaptiveIcon(Comp: LucideIcon): LucideIcon {
  return forwardRef<SVGSVGElement, LucideProps>(({ size, ...rest }, ref) => {
    const level = useSettingsStore((s) => s.adaptiveLevel);
    const m = ADAPTIVE_ICON_MULT[level - 1] ?? 1;
    let effSize: string | number | undefined = size;
    if (typeof size === "number") effSize = Math.round(size * m);
    else if (size === undefined) effSize = Math.round(24 * m); // lucide 默认 24
    return <Comp ref={ref} size={effSize} {...rest} />;
  }) as unknown as LucideIcon;
}

export const Activity = adaptiveIcon(Lucide.Activity);
export const AlertTriangle = adaptiveIcon(Lucide.AlertTriangle);
export const ArrowLeft = adaptiveIcon(Lucide.ArrowLeft);
export const ArrowUpCircle = adaptiveIcon(Lucide.ArrowUpCircle);
export const Check = adaptiveIcon(Lucide.Check);
export const CheckCheck = adaptiveIcon(Lucide.CheckCheck);
export const CheckCircle2 = adaptiveIcon(Lucide.CheckCircle2);
export const ChevronDown = adaptiveIcon(Lucide.ChevronDown);
export const ChevronLeft = adaptiveIcon(Lucide.ChevronLeft);
export const ChevronRight = adaptiveIcon(Lucide.ChevronRight);
export const Clock = adaptiveIcon(Lucide.Clock);
export const Copy = adaptiveIcon(Lucide.Copy);
export const Cpu = adaptiveIcon(Lucide.Cpu);
export const Download = adaptiveIcon(Lucide.Download);
export const Edit3 = adaptiveIcon(Lucide.Edit3);
export const Ellipsis = adaptiveIcon(Lucide.Ellipsis);
export const ExternalLink = adaptiveIcon(Lucide.ExternalLink);
export const Eye = adaptiveIcon(Lucide.Eye);
export const EyeOff = adaptiveIcon(Lucide.EyeOff);
export const FileCog = adaptiveIcon(Lucide.FileCog);
export const FileText = adaptiveIcon(Lucide.FileText);
export const Filter = adaptiveIcon(Lucide.Filter);
export const Globe = adaptiveIcon(Lucide.Globe);
export const HelpCircle = adaptiveIcon(Lucide.HelpCircle);
export const Home = adaptiveIcon(Lucide.Home);
export const House = adaptiveIcon(Lucide.House);
export const Image = adaptiveIcon(Lucide.Image);
export const Keyboard = adaptiveIcon(Lucide.Keyboard);
export const Languages = adaptiveIcon(Lucide.Languages);
export const Loader2 = adaptiveIcon(Lucide.Loader2);
export const Lock = adaptiveIcon(Lucide.Lock);
export const LogIn = adaptiveIcon(Lucide.LogIn);
export const Menu = adaptiveIcon(Lucide.Menu);
export const Mic = adaptiveIcon(Lucide.Mic);
export const MicOff = adaptiveIcon(Lucide.MicOff);
export const Monitor = adaptiveIcon(Lucide.Monitor);
export const MonitorOff = adaptiveIcon(Lucide.MonitorOff);
export const MoreHorizontal = adaptiveIcon(Lucide.MoreHorizontal);
export const Network = adaptiveIcon(Lucide.Network);
export const Package = adaptiveIcon(Lucide.Package);
export const Palette = adaptiveIcon(Lucide.Palette);
export const PhoneOff = adaptiveIcon(Lucide.PhoneOff);
export const Plus = adaptiveIcon(Lucide.Plus);
export const RefreshCw = adaptiveIcon(Lucide.RefreshCw);
export const Scaling = adaptiveIcon(Lucide.Scaling);
export const ScanLine = adaptiveIcon(Lucide.ScanLine);
export const Search = adaptiveIcon(Lucide.Search);
export const Send = adaptiveIcon(Lucide.Send);
export const Settings = adaptiveIcon(Lucide.Settings);
export const Share2 = adaptiveIcon(Lucide.Share2);
export const ShieldAlert = adaptiveIcon(Lucide.ShieldAlert);
export const Signal = adaptiveIcon(Lucide.Signal);
export const Smartphone = adaptiveIcon(Lucide.Smartphone);
export const Square = adaptiveIcon(Lucide.Square);
export const Terminal = adaptiveIcon(Lucide.Terminal);
export const Trash2 = adaptiveIcon(Lucide.Trash2);
export const Upload = adaptiveIcon(Lucide.Upload);
export const UserPlus = adaptiveIcon(Lucide.UserPlus);
export const Users = adaptiveIcon(Lucide.Users);
export const Volume2 = adaptiveIcon(Lucide.Volume2);
export const VolumeX = adaptiveIcon(Lucide.VolumeX);
export const Wifi = adaptiveIcon(Lucide.Wifi);
export const X = adaptiveIcon(Lucide.X);
export const XCircle = adaptiveIcon(Lucide.XCircle);
export const ZoomIn = adaptiveIcon(Lucide.ZoomIn);
export const ZoomOut = adaptiveIcon(Lucide.ZoomOut);

// Mobile 组件使用的 lucide *Icon 后缀别名
export const AlertCircleIcon = adaptiveIcon(Lucide.AlertCircleIcon);
export const CameraIcon = adaptiveIcon(Lucide.CameraIcon);
export const ChevronLeftIcon = adaptiveIcon(Lucide.ChevronLeftIcon);
export const MicIcon = adaptiveIcon(Lucide.MicIcon);
export const MicOffIcon = adaptiveIcon(Lucide.MicOffIcon);
export const MonitorIcon = adaptiveIcon(Lucide.MonitorIcon);
export const MonitorOffIcon = adaptiveIcon(Lucide.MonitorOffIcon);
export const SettingsIcon = adaptiveIcon(Lucide.SettingsIcon);
export const XIcon = adaptiveIcon(Lucide.XIcon);

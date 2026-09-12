// src/components/Common/index.ts — 公共组件统一导出
//
// Button / Text 为自适应封装（读取 settingsStore.adaptiveLevel 偏移 size）。
// 调用方统一从本 barrel 导入：import { Button, Text } from "@/components/Common"。
// 类型 ButtonProps / TextProps 透传自 @radix-ui/themes（封装组件 props 与原生同构）。

export { default as Button } from "./Button";
export { default as Text } from "./Text";
export { default as Tip } from "./Tip";
export type { ButtonProps, TextProps } from "@radix-ui/themes";

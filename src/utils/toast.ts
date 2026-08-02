import toast from "react-hot-toast";

/** 成功提示（非阻塞、自动消失） */
export function toastSuccess(message: string): void {
  toast.success(message, { duration: 3000 });
}

/** 错误提示（时长稍长，便于阅读） */
export function toastError(message: string): void {
  toast.error(message, { duration: 4000 });
}

/** 通用提示 */
export function toastInfo(message: string): void {
  toast(message, { duration: 3000 });
}

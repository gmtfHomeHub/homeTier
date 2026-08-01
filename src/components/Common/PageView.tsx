import { Flex } from "@radix-ui/themes";
import { ReactNode } from "react";

export interface ViewProps {
  header?: ReactNode;
  content?: ReactNode;
  children?: ReactNode | ReactNode[];
}

export function View({ header, content, children }: ViewProps) {

  return (
    <Flex direction="column" className="flex-1">
      {header ? (<Flex align="center" gap="2" className="p-4 border-b border-[var(--color-border)] bg-[var(--color-surface)] shrink-0">
        {header}
      </Flex>) : null}
      {children}
      {content}
    </Flex>
  );
};
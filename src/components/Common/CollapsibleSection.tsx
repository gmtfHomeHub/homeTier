import React, { useState } from "react";
import { Flex, Text, Card } from "@radix-ui/themes";
import { ChevronDown } from "lucide-react";

interface CollapsibleSectionProps {
  title: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
  className?: string;
}

export const CollapsibleSection: React.FC<CollapsibleSectionProps> = ({
  title,
  children,
  defaultOpen = true,
  className = "",
}) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={`w-full ${className}`}>
      <div
        className="w-full"
        onClick={() => setIsOpen(!isOpen)}
        style={{ cursor: "pointer" }}
      >
        <div className="flex items-center justify-between p-4 border-b border-[var(--color-border)]">
          <Text size="2" weight="medium">
            {title}
          </Text>
          <span className="transition-transform duration-200" style={{ transform: `rotate(${isOpen ? 0 : -90}deg)` }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </span>
        </div>
      </div>
      <div
        style={{
          overflow: "hidden",
          transition: "max-height 0.3s ease, opacity 0.3s ease",
          maxHeight: isOpen ? "none" : 0,
          opacity: isOpen ? 1 : 0,
        }}
      >
        <div className="pt-4">{children}</div>
      </div>
    </div>
  );
};

export default CollapsibleSection;

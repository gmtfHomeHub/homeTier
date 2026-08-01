import { ReactElement } from "react";
import { Table } from "@radix-ui/themes";

type AlignProp = "left" | "center" | "right" | "char" | undefined;
type JustifyProp = "start" | "center" | "end";

const ALIGNMAP = {
  "left": "start",
  "right": "end",
};

export interface ColumnProps<T, K extends keyof T = keyof T> {
  title: string;
  align?: Omit<AlignProp, "char" | "right"> | "start" | "end";
  field: K;
  /** render 第一个参数类型 = T[field]，由 field 自动收紧 */
  render?(value: T[K], record: T, index: number): unknown;
  minWidth?: string;
}

// 生成所有可能的列配置联合
export type ColumnDefs<T> = { [K in keyof T]: ColumnProps<T, K> }[keyof T];

export interface BaseTableProps<T> {
  columns: readonly ColumnDefs<T>[];
  dataSource: T[];
  children?: ReactElement;
}

const TABLE_CELL_MIN_WIDTH = "90px";


const defaultProps = {
  minWidth: TABLE_CELL_MIN_WIDTH,
}

export function BaseTable<T>({ columns, dataSource, children }: BaseTableProps<T>) {
  return (
    <Table.Root variant="surface" size="1">
      <Table.Header>
        <Table.Row>
          {columns.map((col, i) => {
            const { title, align, render, field, ...props } = col as ColumnProps<T, keyof T>;
            return (
              <Table.ColumnHeaderCell {...{...defaultProps, ...props}} key={i} justify={(align ? (ALIGNMAP[align as "left" | "right"] as JustifyProp) : align) || "center"}>
                {title}
              </Table.ColumnHeaderCell>
            );
          })}
        </Table.Row>
      </Table.Header>

      <Table.Body>
        {dataSource.length === 0 ? children : dataSource.map((record, index) => (
          <Table.Row key={index}>
            {columns.map((col, i) => {
              const { render, field, align, ...props } = col as ColumnProps<T, keyof T>;
              const cell = (render?.(record[field], record, index) || record[field]) as ReactElement || '-';
              return i === 0 ? (
                <Table.RowHeaderCell {...{...defaultProps, ...props}} key={`${index}_${i}`} align={align as AlignProp || "center"}>
                  {cell}
                </Table.RowHeaderCell>
              ) : (
                <Table.Cell {...{...defaultProps, ...props}} key={`${index}_${i}`} align={align as AlignProp || 'center'}>
                  {cell}
                </Table.Cell>
              );
            })}
          </Table.Row>
        ))}
      </Table.Body>
    </Table.Root>
  );
}

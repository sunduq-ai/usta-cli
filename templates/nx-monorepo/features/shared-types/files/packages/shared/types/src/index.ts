/**
 * Shared types — populated as the project grows. The package is wired into
 * the workspace via `tsconfig.base.json` (`paths`) and pnpm workspaces.
 */

export type Id = string;

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

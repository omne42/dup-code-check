export interface ScanOptions {
  ignore_dirs?: string[];
  max_file_size?: number;
  cross_repo_only?: boolean;
  follow_symlinks?: boolean;
}

export interface DuplicateFile {
  repo_id: number;
  repo_label: string;
  path: string;
}

export interface DuplicateGroup {
  hash: string;
  normalized_len: number;
  files: DuplicateFile[];
}

export function findDuplicateFiles(
  roots: string[],
  options?: ScanOptions
): DuplicateGroup[];


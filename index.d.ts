export interface ScanOptions {
  ignoreDirs?: string[];
  maxFileSize?: number;
  minMatchLen?: number;
  minTokenLen?: number;
  similarityThreshold?: number;
  simhashMaxDistance?: number;
  maxReportItems?: number;
  respectGitignore?: boolean;
  crossRepoOnly?: boolean;
  followSymlinks?: boolean;
}

export interface DuplicateFile {
  repoId: number;
  repoLabel: string;
  path: string;
}

export interface DuplicateGroup {
  hash: string;
  normalizedLen: number;
  files: DuplicateFile[];
}

export interface DuplicateSpanOccurrence {
  repoId: number;
  repoLabel: string;
  path: string;
  startLine: number;
  endLine: number;
}

export interface DuplicateSpanGroup {
  hash: string;
  normalizedLen: number;
  preview: string;
  occurrences: DuplicateSpanOccurrence[];
}

export interface SimilarityPair {
  a: DuplicateSpanOccurrence;
  b: DuplicateSpanOccurrence;
  score: number;
  distance?: number | null;
}

export interface DuplicationReport {
  fileDuplicates: DuplicateGroup[];
  codeSpanDuplicates: DuplicateSpanGroup[];
  lineSpanDuplicates: DuplicateSpanGroup[];
  tokenSpanDuplicates: DuplicateSpanGroup[];
  blockDuplicates: DuplicateSpanGroup[];
  astSubtreeDuplicates: DuplicateSpanGroup[];
  similarBlocksMinhash: SimilarityPair[];
  similarBlocksSimhash: SimilarityPair[];
}

export function findDuplicateFiles(
  roots: string[],
  options?: ScanOptions
): DuplicateGroup[];

export function findDuplicateCodeSpans(
  roots: string[],
  options?: ScanOptions
): DuplicateSpanGroup[];

export function generateDuplicationReport(
  roots: string[],
  options?: ScanOptions
): DuplicationReport;

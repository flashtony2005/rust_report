// Report Studio Type Definitions

export type DriverType = 'postgres' | 'mysql' | 'sqlite';

export interface Datasource {
  id: string;
  label: string;
  driver: DriverType;
  driver_label?: string;
  host?: string;
  port?: string | number;
  database: string;
  user?: string;
  password?: string;
  has_password?: boolean;
  options?: string;
}

export interface ColumnMeta {
  type: string;
  comment?: string;
  sample?: any;
}

export interface Dataset {
  kind: 'sample' | 'file' | 'sql';
  file?: string;
  dsn?: string;
  sql?: string;
  rows: number;
  columns: string[];
  column_meta?: Record<string, ColumnMeta>;
  preview?: any[][];
  encoding?: string;
  sample_rows?: number;
}

export interface Dimension {
  field: string;
  label?: string;
  sort?: 'none' | 'asc' | 'desc';
}

export interface Measure {
  field: string;
  agg: 'sum' | 'avg' | 'max' | 'min' | 'count';
  label: string;
  format?: string;
}

export interface Totals {
  grand: boolean;
  subtotal: boolean;
}

export type ReportShape = 'detail' | 'grouped' | 'crosstab' | 'trend' | 'rank' | 'compare';

export interface Spec {
  layout?: 'list' | 'grouped' | 'crosstab';
  shape?: ReportShape;
  row_dims: Dimension[];
  col_dims: Dimension[];
  measures: Measure[];
  totals: Totals;
  filters?: any[];
}

export interface ShapeDecision {
  shape: ReportShape;
  label: string;
  reasons: string[];
  blockers: string[];
  alternatives?: string[];
}

export interface CellBorder {
  style?: string;
  top?: boolean | { style?: string };
  bottom?: boolean | { style?: string };
  left?: boolean | { style?: string };
  right?: boolean | { style?: string };
}

export interface CellMerge {
  rows: number;
  cols: number;
}

export interface TemplateCell {
  row: number;
  col: number;
  text?: string;
  expr?: string;
  formula?: string;
  bold?: boolean;
  font_size?: number;
  fore_color?: string;
  back_color?: string;
  h_align?: number | 'left' | 'center' | 'right';
  v_align?: number | 'top' | 'middle' | 'bottom';
  border?: CellBorder | null;
  merge?: CellMerge;
}

export interface Template {
  rev: number;
  grid: { rows: number; cols: number };
  cells: TemplateCell[];
  geometry?: { rows: number; cols: number };
  lint?: { summary?: { total?: number } };
}

export interface UniverCellData {
  v?: string | number | boolean;
  t?: number; // 1: string, 2: number, 3: boolean
  s?: {
    bl?: number; // bold (1)
    it?: number; // italic (1)
    ul?: { s?: number }; // underline
    ff?: string; // font family
    fs?: number; // font size
    cl?: { rgb?: string }; // color
    bg?: { rgb?: string }; // background color
    ht?: number; // horizontal align: 1: left, 2: center, 3: right
    vt?: number; // vertical align: 1: top, 2: middle, 3: bottom
    bd?: {
      b?: { s?: number; cl?: { rgb?: string } };
      t?: { s?: number; cl?: { rgb?: string } };
      l?: { s?: number; cl?: { rgb?: string } };
      r?: { s?: number; cl?: { rgb?: string } };
    };
  };
  f?: string; // formula expression
}

export interface IWorksheetData {
  id: string;
  name: string;
  rowCount: number;
  columnCount: number;
  defaultRowHeight?: number;
  defaultColumnWidth?: number;
  showGridlines?: number;
  freeze?: {
    xSplit?: number;
    ySplit?: number;
    startRow?: number;
    startColumn?: number;
  };
  cellData: Record<string, Record<string, UniverCellData>>;
  mergeData?: Array<{
    startRow: number;
    endRow: number;
    startColumn: number;
    endColumn: number;
  }>;
  rowData?: Record<string, { h?: number }>;
  columnData?: Record<string, { w?: number }>;
}

export interface IWorkbookData {
  id: string;
  name: string;
  appVersion?: string;
  locale?: string;
  sheetOrder: string[];
  sheets: Record<string, IWorksheetData>;
  styles?: Record<string, any>;
}

export interface Candidate {
  id: string;
  name: string;
  shape_label: string;
  source: 'library' | 'generated';
  score?: number;
  reasons: string[];
  missing: string[];
  fields?: string[];
}

export interface RunInput {
  kind: string;
  dataset?: string;
  sha256?: string;
}

export interface Run {
  run_id: string;
  req_id: string;
  req_title: string;
  status: 'succeeded' | 'failed';
  template_rev: number;
  duration_ms: number;
  shape: { rows: number; cols: number };
  finished_at: string;
  at: string;
  engine_version: string;
  layout?: { page_count: number };
  parameters: Record<string, any>;
  inputs: RunInput[];
  artifacts: Record<string, any>;
  univerSnapshot?: IWorkbookData;
  computedData?: {
    headers: string[];
    rows: any[][];
    summaryRow?: any[];
  };
}

export interface SemanticVersion {
  version: number;
  saved_at: string;
  note: string;
  template_rev: number;
  shape: string;
  semantic: {
    hash: string;
    calibers: any[];
  };
}

export interface Schedule {
  id: string;
  requirement: string;
  kind: 'daily' | 'weekly' | 'hourly' | 'monthly';
  at?: string;
  weekday?: number;
  day?: number;
  interval_hours?: number;
  enabled: boolean;
  next_run?: string;
  last_status?: string;
  runs: number;
}

export interface Alert {
  id: string;
  req_id?: string;
  dsn?: string;
  metric?: string;
  metric_label?: string;
  window?: { cur_start: string; cur_end: string };
  at: string;
  level: 'warn' | 'critical';
  message: string;
}

export interface AnalysisFinding {
  what: string;
  kind: 'observed' | 'computed';
  evidence: string[];
  share?: number;
}

export interface AnalysisStep {
  title: string;
  kind: string;
  row_count: number;
  ok: boolean;
  intent: string;
  sql: string;
}

export interface AnalysisDoc {
  status: string;
  summary: string;
  totals: { cur: number; prev: number; delta: number };
  coverage: number;
  windows: {
    cur_start: string;
    cur_end: string;
    prev_start: string;
    prev_end: string;
  };
  findings: AnalysisFinding[];
  steps: AnalysisStep[];
  caveats: string[];
}

export interface Requirement {
  id: string;
  title: string;
  description: string;
  dataset: Dataset;
  roles: { measures: string[] };
  template_choice?: Candidate;
  spec?: Spec;
  shape_decision?: ShapeDecision;
  analysis?: any;
  analysis_stale?: boolean;
  template?: Template;
  runs: Run[];
  messages?: { role: 'user' | 'agent'; text: string }[];
  status: 'draft' | 'analyzed' | 'compiled' | 'ran' | 'accepted';
  status_label: string;
  updated_at: string;
}

export interface LlmConfig {
  provider: 'deepseek' | 'openai';
  base_url: string;
  model: string;
  api_key?: string;
  has_key?: boolean;
}

export interface ShapeInfo {
  shape: ReportShape;
  label: string;
  layout: 'list' | 'grouped' | 'crosstab';
  needs: string[];
  template_shapes: string[];
}

export interface BizMetric {
  id: string;
  label: string;
  entity: string;
  column?: string;
  expression: string;
  filters: string[];
  time_field?: string | null;
  grain?: string;
  format?: string;
  status: 'confirmed' | 'candidate' | 'draft';
  evidence: string[];
  aliases: string[];
}

export interface BizDimension {
  id: string;
  entity: string;
  label: string;
  kind: 'enum' | 'text' | 'date' | 'number';
  values?: string[];
  time_grain?: string | null;
  status: 'confirmed' | 'candidate' | 'draft';
  evidence: string[];
  aliases?: string[];
}

export interface BizRule {
  id: string;
  label: string;
  expression: string;
  applies_to: string[];
  status: 'confirmed' | 'candidate' | 'draft';
  evidence: string[];
}

export interface BizEntity {
  id: string;
  table: string;
  label: string;
  grain: {
    key: string[];
    description: string;
    status: 'confirmed' | 'candidate' | 'draft';
    evidence: string[];
  };
  status: 'confirmed' | 'candidate' | 'draft';
  evidence: string[];
}

export interface BizRelationship {
  id: string;
  label?: string;
  from_entity: string;
  to_entity: string;
  cardinality: '1:1' | '1:N' | 'N:1' | 'M:N';
  from_key: string;
  to_key: string;
  status: 'confirmed' | 'candidate' | 'draft';
  evidence?: string[];
}

export interface OntologyResolutionResult {
  intent: string;
  requestedDimensions: string[];
  requestedMetrics: string[];
  rootEntity: string;
  resolvedEntities: string[];
  joinPath: Array<{
    from: string;
    to: string;
    cardinality: string;
    joinOn: string;
  }>;
  fanOutProtected: boolean;
  fanOutPreventionNotes: string[];
  polarsCode: string;
  grainAlignedSql: string;
  outputData: {
    headers: string[];
    rows: any[][];
  };
  univerSnapshot: IWorkbookData;
  identifiedTokens?: Array<{
    token: string;
    type: 'dim' | 'metric' | 'entity';
    label: string;
    targetEntity: string;
    detail: string;
  }>;
  fanOutComparison?: {
    badExample: {
      headers: string[];
      rows: any[][];
      description: string;
    };
    goodExample: {
      headers: string[];
      rows: any[][];
      description: string;
    };
  };
}

export interface BizModel {
  schema: string;
  version: number;
  source: {
    dsn: string;
    generated_at: string;
    generator: string;
  };
  entities: BizEntity[];
  relationships?: BizRelationship[];
  metrics: BizMetric[];
  dimensions: BizDimension[];
  rules: BizRule[];
  open_questions?: { question: string; source: string; evidence: string[] }[];
}

export interface GoldenQuestion {
  id: string;
  intent: string;
  query: {
    metrics: { ref: string; label?: string }[];
    dimensions: { ref: string; time_grain?: string }[];
    filters?: { ref: string; op: string; value: any }[];
    sort?: { by: { metric: string }; direction: 'asc' | 'desc' }[];
  };
  expect: {
    rows?: number;
    cols?: number;
    shape?: string;
    note?: string;
  };
}

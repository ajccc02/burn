import type { CATEGORIES } from "./constants";
export type Category = (typeof CATEGORIES)[number];
export type ReportType = "lost" | "found";
export type ReportStatus = "active" | "resolved";
export type Report = {
  id: string;
  user_id: string | null;
  report_type: ReportType;
  item_name: string;
  category: Category;
  description: string;
  color: string | null;
  location: string;
  incident_date: string;
  image_path: string | null;
  additional_info: string | null;
  turned_over: boolean;
  status: ReportStatus;
  is_demo: boolean;
  created_at: string;
  updated_at: string;
};
export type DisplayReport = Report & { image_url: string | null };
export interface MatchSuggestion {
  report: DisplayReport;
  score: number;
  reasons: string[];
}
export type Result<T> =
  | { ok: true; data: T }
  | {
      ok: false;
      error: string;
      fields?: Record<string, string>;
      code?: string;
    };
export interface ReportFilters {
  q: string;
  type: "all" | ReportType;
  category: string;
  location: string;
  status: "all" | ReportStatus;
  sort: "recent" | "oldest";
  page: number;
}
export interface Feed {
  reports: DisplayReport[];
  total: number;
  locations: string[];
}
export interface Counts {
  lost: number;
  found: number;
  resolved: number;
  demo: number;
}
export type ReportInsert = Omit<Report, "created_at" | "updated_at"> & {
  created_at?: string;
  updated_at?: string;
};
export type FeedbackRecord = {
  id: string;
  participant_code: string;
  testing_stage: "Initial" | "Showcase";
  consent_given: boolean;
  ease_of_use: number;
  navigation: number;
  clarity: number;
  system_feedback: number;
  accessibility: number;
  task_completion: number;
  overall_satisfaction: number;
  comments: string | null;
  created_at?: string;
};
export interface Database {
  public: {
    Tables: {
      reports: {
        Row: Report;
        Insert: ReportInsert;
        Update: Partial<ReportInsert>;
        Relationships: [];
      };
      usability_feedback: {
        Row: FeedbackRecord;
        Insert: FeedbackRecord;
        Update: never;
        Relationships: [];
      };
    };
    Views: Record<string, never>;
    Functions: {
      search_reports: { Args: { search_term?: string }; Returns: Report[] };
      report_counts: { Args: Record<string, never>; Returns: Counts };
      report_locations: { Args: Record<string, never>; Returns: string[] };
      match_candidates: { Args: { source_id: string }; Returns: Report[] };
    };
    Enums: Record<string, never>;
    CompositeTypes: Record<string, never>;
  };
}

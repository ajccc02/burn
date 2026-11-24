export const CATEGORIES = [
  "Electronics",
  "School Supplies",
  "Bags",
  "Identification Cards",
  "Keys",
  "Wallets",
  "Clothing",
  "Accessories",
  "Water Bottles",
  "Others",
] as const;
export const LOCATIONS = [
  "Main Building",
  "University Library",
  "Canteen",
  "Gymnasium",
  "Student Lounge",
  "Computer Laboratory",
  "Engineering Building",
  "Campus Quadrangle",
] as const;
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const IMAGE_BUCKET = "report-images";
export const CAMPUS_TIME_ZONE = "Asia/Manila";
export const QUESTIONS = [
  ["ease_of_use", "The application was easy to use."],
  ["navigation", "I could navigate the application without confusion."],
  ["clarity", "The labels and instructions were clear."],
  [
    "system_feedback",
    "The application provided appropriate feedback after my actions.",
  ],
  ["accessibility", "The interface was readable and accessible."],
  ["task_completion", "I successfully completed the assigned task."],
  ["overall_satisfaction", "Overall, I am satisfied with the application."],
] as const;
export const SCALE = [
  "Strongly disagree",
  "Disagree",
  "Neutral",
  "Agree",
  "Strongly agree",
];

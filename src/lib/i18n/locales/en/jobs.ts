/**
 * jobs namespace strings (filled by migration subagent).
 */
import type { jobsZh } from "../zh/jobs";

export const jobsEn: Record<keyof typeof jobsZh, string> = {
  // Page
  "jobs.title": "Jobs",
  "jobs.count": "{n}",
  "jobs.create": "New Job",
  "jobs.searchPlaceholder": "Search job name...",
  "jobs.loadError": "Failed to load jobs",
  "jobs.empty.noMatch": "No matching jobs found",
  "jobs.empty.clearSearch": "Clear search",
  "jobs.empty.none": "No jobs created yet",
  "jobs.empty.hint": "Click the button above to create your first scheduled job",
  "jobs.delete.confirmTitle": "Delete Job",
  "jobs.delete.confirmMessage":
    "Are you sure you want to delete this job? This action cannot be undone.",
  "jobs.delete.failed": "Delete failed, please try again",

  // Card
  "jobs.target.artifact": "Artifact",
  "jobs.target.agent": "Agent",
  "jobs.target.prompt": "Prompt",
  "jobs.view.aria": "View job {name} details",
  "jobs.edit.aria": "Edit job {name}",
  "jobs.delete.aria": "Delete job {name}",
  "jobs.card.nextRun": "Next run: ",
  "jobs.card.lastStatus": "Last status: ",
  "jobs.card.neverRun": "Never run",
  "jobs.card.runCount": "Runs: {n}",
  "jobs.card.failureCount": "{n} failures",

  // Execution status
  "jobs.status.running": "Running",
  "jobs.status.success": "Success",
  "jobs.status.failed": "Failed",
  "jobs.status.timeout": "Timed out",
  "jobs.status.disabled": "Disabled",

  // Trigger
  "jobs.trigger.schedule": "Scheduled",
  "jobs.trigger.manual": "Manual",

  // Detail
  "jobs.detail.title": "Job Details",
  "jobs.detail.nextRun": "Next run: ",
  "jobs.detail.runCount": "{n} runs",
  "jobs.detail.failureCount": "{n} failures",
  "jobs.detail.history": "Execution History",
  "jobs.detail.runNow": "Run Now",
  "jobs.detail.runningNow": "Running…",
  "jobs.detail.runNowFailed": "Run failed, please try again",
  "jobs.detail.historyLoadError":
    "Failed to load execution history, please try again",
  "jobs.detail.emptyHistory": "No execution records",
  "jobs.detail.runningDuration": "Running",
  "jobs.detail.resultUnavailable": "Result unavailable",
  "jobs.detail.checkingResult": "Checking result…",
  "jobs.detail.jumpToResult": "Go to result",

  // Form
  "jobs.form.editTitle": "Edit Job",
  "jobs.form.createTitle": "New Job",
  "jobs.form.appClosedNotice":
    "Scheduled jobs only trigger while the app is running; they will not execute while the app is closed.",
  "jobs.form.sectionBasic": "Basic Info",
  "jobs.form.sectionSchedule": "Schedule",
  "jobs.form.sectionTarget": "Target",
  "jobs.form.sectionAdvanced": "Advanced",
  "jobs.form.name": "Name",
  "jobs.form.namePlaceholder": "Enter job name",
  "jobs.form.nameRequired": "Please enter a job name",
  "jobs.form.descriptionOptional": "Description (optional)",
  "jobs.form.descriptionPlaceholder": "Enter job description…",
  "jobs.form.execTimeout": "Timeout (seconds)",
  "jobs.form.execTimeoutPlaceholder":
    "Leave empty to use default {n} (0 means no timeout)",
  "jobs.form.execTimeoutHint":
    "0 means no timeout; leave empty to use the default.",
  "jobs.form.execTimeoutLabel": "Timeout",
  "jobs.form.maxRetries": "Max retries",
  "jobs.form.maxRetriesPlaceholder":
    "Leave empty to use default {n} (0 means no retry)",
  "jobs.form.maxRetriesHint":
    "0 means no retry after failure; leave empty to use the default.",
  "jobs.form.maxRetriesLabel": "Max retries",
  "jobs.form.retryDelay": "Retry interval (seconds)",
  "jobs.form.retryDelayPlaceholder": "Leave empty to use default {n}",
  "jobs.form.retryDelayHint": "Leave empty to use the default {n} seconds.",
  "jobs.form.retryDelayLabel": "Retry interval",
  "jobs.form.mustBeInteger": "{label} must be an integer",
  "jobs.form.mustNotBeNegative": "{label} cannot be negative",
  "jobs.form.saveFailed": "Save failed, please try again",
  "jobs.form.saving": "Saving…",
  "jobs.form.save": "Save",
  "jobs.form.createAction": "Create",

  // Schedule editor
  "jobs.schedule.tabQuick": "Quick",
  "jobs.schedule.tabAdvanced": "Advanced Cron",
  "jobs.schedule.frequency": "Frequency",
  "jobs.schedule.presetMinutes": "Every N minutes",
  "jobs.schedule.presetHours": "Every N hours",
  "jobs.schedule.presetDaily": "Daily",
  "jobs.schedule.presetWeekly": "Weekly",
  "jobs.schedule.presetMonthly": "Monthly",
  "jobs.schedule.everyMinutes": "Every how many minutes",
  "jobs.schedule.everyHours": "Every how many hours",
  "jobs.schedule.time": "Time",
  "jobs.schedule.weekdays": "Weekdays (multi-select)",
  "jobs.schedule.dayOfMonth": "Day of month",
  "jobs.schedule.weekday.sun": "Sun",
  "jobs.schedule.weekday.mon": "Mon",
  "jobs.schedule.weekday.tue": "Tue",
  "jobs.schedule.weekday.wed": "Wed",
  "jobs.schedule.weekday.thu": "Thu",
  "jobs.schedule.weekday.fri": "Fri",
  "jobs.schedule.weekday.sat": "Sat",
  "jobs.schedule.cronLabel":
    "Cron expression (standard 5 fields: min hour day month weekday)",
  "jobs.schedule.cronRequired": "Please enter a cron expression",
  "jobs.schedule.previewTitle": "Next {n} executions (local time)",
  "jobs.schedule.previewFailed": "Preview failed",
  "jobs.schedule.calculating": "Calculating…",
  "jobs.schedule.noOccurrences": "No upcoming executions",

  // Target picker
  "jobs.target.kindLabel": "Target type",
  "jobs.target.artifactLabel": "Artifact to run",
  "jobs.target.artifactLoading": "Loading Artifact list…",
  "jobs.target.artifactEmpty":
    "No installed Artifacts, please install one on the Artifact page first.",
  "jobs.target.artifactSelect": "Please select an Artifact",
  "jobs.target.artifactRequired": "Please select an Artifact",
  "jobs.target.argsLabel": "Command-line arguments",
  "jobs.target.argsAdd": "Add argument",
  "jobs.target.argsEmpty": "No extra arguments",
  "jobs.target.argAria": "Argument {n}",
  "jobs.target.argRemoveAria": "Remove argument {n}",
  "jobs.target.argPlaceholder": "--flag or value",
  "jobs.target.envLabel": "Environment variables",
  "jobs.target.envAdd": "Add variable",
  "jobs.target.envEmpty": "No environment variables",
  "jobs.target.envKeyAria": "Environment variable name {n}",
  "jobs.target.envValueAria": "Environment variable value {n}",
  "jobs.target.envRemoveAria": "Remove environment variable {n}",
  "jobs.target.modelLabel": "Model",
  "jobs.target.modelRequired": "Please select a provider and model",
  "jobs.target.promptLabel": "Prompt text",
  "jobs.target.promptAria": "Prompt text",
  "jobs.target.promptPlaceholder": "Enter the prompt to send to the model…",
  "jobs.target.promptRequired": "Please enter the prompt text",
  "jobs.target.agentLabel": "Agent template",
  "jobs.target.agentLoading": "Loading Agent list…",
  "jobs.target.agentEmpty":
    "No Agent templates, please create one on the Agents page first.",
  "jobs.target.agentSelect": "Please select an Agent",
  "jobs.target.agentRequired": "Please select an Agent",
  "jobs.target.initialMessageLabel": "Initial instruction (optional)",
  "jobs.target.initialMessageAria": "Initial instruction",
  "jobs.target.initialMessagePlaceholder":
    "Initial instruction to run after the Agent starts…",
};

import { invoke } from "@tauri-apps/api/core";

enum LogLevel {
  Info = 3,
  Warn = 4,
  Error = 5,
}

const MAX_MESSAGE_LENGTH = 8_000;

let installed = false;

function stringifyArg(value: unknown): string {
  if (value instanceof Error) {
    return `${value.name}: ${value.message}${value.stack ? `\n${value.stack}` : ""}`;
  }

  if (typeof value === "string") {
    return value;
  }

  if (value === undefined) {
    return "undefined";
  }

  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function formatArgs(args: unknown[]): string {
  const message = args.map(stringifyArg).join(" ");
  return message.length > MAX_MESSAGE_LENGTH
    ? `${message.slice(0, MAX_MESSAGE_LENGTH)}... [truncated]`
    : message;
}

async function writeFrontendLog(
  level: LogLevel,
  message: string,
  location: string,
): Promise<void> {
  try {
    await invoke("plugin:log|log", {
      level,
      message,
      location,
      keyValues: {
        source: "frontend",
      },
    });
  } catch {
    // Logging must never break the application path that is being logged.
  }
}

export function installFrontendLogger(): void {
  if (installed || typeof window === "undefined") {
    return;
  }
  installed = true;

  const originalError = console.error.bind(console);
  const originalWarn = console.warn.bind(console);
  const originalInfo = console.info.bind(console);

  console.error = (...args: unknown[]) => {
    originalError(...args);
    void writeFrontendLog(LogLevel.Error, formatArgs(args), "console.error");
  };

  console.warn = (...args: unknown[]) => {
    originalWarn(...args);
    void writeFrontendLog(LogLevel.Warn, formatArgs(args), "console.warn");
  };

  console.info = (...args: unknown[]) => {
    originalInfo(...args);
    void writeFrontendLog(LogLevel.Info, formatArgs(args), "console.info");
  };

  window.addEventListener("error", (event) => {
    const message = event.error
      ? stringifyArg(event.error)
      : `${event.message} at ${event.filename}:${event.lineno}:${event.colno}`;
    void writeFrontendLog(LogLevel.Error, message, "window.error");
  });

  window.addEventListener("unhandledrejection", (event) => {
    void writeFrontendLog(
      LogLevel.Error,
      `Unhandled promise rejection: ${stringifyArg(event.reason)}`,
      "window.unhandledrejection",
    );
  });
}

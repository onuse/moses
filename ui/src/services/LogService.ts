import { LogEntry } from '../components/LogConsole';

export class LogService {
  private static instance: LogService;
  private logs: LogEntry[] = [];
  private listeners: Set<(logs: LogEntry[]) => void> = new Set();
  private maxLogs = 10000; // Keep last 10k logs

  private constructor() {}

  static getInstance(): LogService {
    if (!LogService.instance) {
      LogService.instance = new LogService();
    }
    return LogService.instance;
  }

  addLog(level: LogEntry['level'], message: string, source?: string) {
    const entry: LogEntry = {
      timestamp: new Date(),
      level,
      message,
      source
    };

    this.logs.push(entry);
    
    // Trim logs if too many
    if (this.logs.length > this.maxLogs) {
      this.logs = this.logs.slice(-this.maxLogs);
    }

    this.notifyListeners();
  }

  debug(message: string, source?: string) {
    this.addLog('DEBUG', message, source);
  }

  info(message: string, source?: string) {
    this.addLog('INFO', message, source);
  }

  warn(message: string, source?: string) {
    this.addLog('WARN', message, source);
  }

  error(message: string, source?: string) {
    this.addLog('ERROR', message, source);
  }

  getLogs(): LogEntry[] {
    return [...this.logs];
  }

  clear() {
    this.logs = [];
    this.notifyListeners();
  }

  subscribe(listener: (logs: LogEntry[]) => void) {
    this.listeners.add(listener);
    // Immediately call with current logs
    listener(this.getLogs());
    
    // Return unsubscribe function
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notifyListeners() {
    const logs = this.getLogs();
    this.listeners.forEach(listener => listener(logs));
  }
}

// Create singleton instance
export const logService = LogService.getInstance();
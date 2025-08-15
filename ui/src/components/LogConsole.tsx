import React, { useEffect, useRef, useState } from 'react';
import { ChevronUp, ChevronDown, Copy, Trash2, Download } from 'lucide-react';

export interface LogEntry {
  timestamp: Date;
  level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
  message: string;
  source?: string;
}

interface LogConsoleProps {
  logs: LogEntry[];
  onClear?: () => void;
  maxHeight?: string;
}

export const LogConsole: React.FC<LogConsoleProps> = ({ 
  logs, 
  onClear,
  maxHeight = '300px' 
}) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const [filter, setFilter] = useState<'ALL' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'>('ALL');
  const [autoScroll, setAutoScroll] = useState(true);
  const consoleRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoScroll && consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const filteredLogs = logs.filter(log => 
    filter === 'ALL' || log.level === filter
  );

  const getLevelColor = (level: LogEntry['level']) => {
    switch (level) {
      case 'DEBUG': return 'text-gray-400';
      case 'INFO': return 'text-blue-400';
      case 'WARN': return 'text-yellow-400';
      case 'ERROR': return 'text-red-400';
    }
  };

  const formatTimestamp = (date: Date) => {
    return date.toLocaleTimeString('en-US', { 
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      fractionalSecondDigits: 3
    });
  };

  const copyLogs = () => {
    const text = filteredLogs
      .map(log => `[${formatTimestamp(log.timestamp)}] [${log.level}] ${log.source ? `[${log.source}] ` : ''}${log.message}`)
      .join('\n');
    navigator.clipboard.writeText(text);
  };

  const downloadLogs = () => {
    const text = logs
      .map(log => `[${formatTimestamp(log.timestamp)}] [${log.level}] ${log.source ? `[${log.source}] ` : ''}${log.message}`)
      .join('\n');
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `moses-logs-${new Date().toISOString()}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="bg-gray-900 border border-gray-700 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="bg-gray-800 px-4 py-2 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-gray-400 hover:text-white transition-colors"
          >
            {isExpanded ? <ChevronDown size={20} /> : <ChevronUp size={20} />}
          </button>
          <span className="text-sm font-mono text-gray-300">
            Console Output ({filteredLogs.length} logs)
          </span>
        </div>

        <div className="flex items-center gap-2">
          {/* Filter buttons */}
          <div className="flex gap-1 mr-4">
            {(['ALL', 'DEBUG', 'INFO', 'WARN', 'ERROR'] as const).map(level => (
              <button
                key={level}
                onClick={() => setFilter(level)}
                className={`px-2 py-1 text-xs font-mono rounded transition-colors ${
                  filter === level 
                    ? 'bg-gray-700 text-white' 
                    : 'text-gray-400 hover:text-white'
                }`}
              >
                {level}
              </button>
            ))}
          </div>

          {/* Auto-scroll toggle */}
          <label className="flex items-center gap-1 text-xs text-gray-400">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
              className="rounded border-gray-600"
            />
            Auto-scroll
          </label>

          {/* Action buttons */}
          <button
            onClick={copyLogs}
            className="p-1 text-gray-400 hover:text-white transition-colors"
            title="Copy logs"
          >
            <Copy size={16} />
          </button>
          <button
            onClick={downloadLogs}
            className="p-1 text-gray-400 hover:text-white transition-colors"
            title="Download all logs"
          >
            <Download size={16} />
          </button>
          {onClear && (
            <button
              onClick={onClear}
              className="p-1 text-gray-400 hover:text-red-400 transition-colors"
              title="Clear logs"
            >
              <Trash2 size={16} />
            </button>
          )}
        </div>
      </div>

      {/* Console */}
      {isExpanded && (
        <div
          ref={consoleRef}
          className="bg-black p-2 overflow-y-auto font-mono text-xs"
          style={{ maxHeight }}
          onScroll={() => {
            if (consoleRef.current) {
              const { scrollTop, scrollHeight, clientHeight } = consoleRef.current;
              // Disable auto-scroll if user scrolls up
              if (scrollTop < scrollHeight - clientHeight - 10) {
                setAutoScroll(false);
              }
            }
          }}
        >
          {filteredLogs.length === 0 ? (
            <div className="text-gray-600 italic">No logs to display</div>
          ) : (
            filteredLogs.map((log, index) => (
              <div key={index} className="py-0.5 hover:bg-gray-900">
                <span className="text-gray-600">[{formatTimestamp(log.timestamp)}]</span>
                {' '}
                <span className={`${getLevelColor(log.level)} font-semibold`}>
                  [{log.level}]
                </span>
                {log.source && (
                  <>
                    {' '}
                    <span className="text-purple-400">[{log.source}]</span>
                  </>
                )}
                {' '}
                <span className="text-gray-300 whitespace-pre-wrap">{log.message}</span>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
};
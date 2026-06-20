import React, { useState, useEffect } from 'react';
import { FileText, Search, Filter, Download, Clock } from 'lucide-react';
import { AuditLogEntry } from '../lib/adminTypes';

function AuditLogViewer() {
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  const [filteredLogs, setFilteredLogs] = useState<AuditLogEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [actionFilter, setActionFilter] = useState<string>('all');
  const [contractFilter, setContractFilter] = useState<string>('all');

  useEffect(() => {
    loadLogs();
  }, []);

  useEffect(() => {
    filterLogs();
  }, [logs, searchQuery, actionFilter, contractFilter]);

  const loadLogs = () => {
    try {
      const stored = localStorage.getItem('adminAuditLogs');
      if (stored) {
        setLogs(JSON.parse(stored));
      }
    } catch (err) {
      console.error('Failed to load audit logs:', err);
      setLogs([]);
    }
  };

  const filterLogs = () => {
    let filtered = [...logs];

    if (searchQuery) {
      filtered = filtered.filter(
        (log) =>
          log.action.toLowerCase().includes(searchQuery.toLowerCase()) ||
          log.admin.toLowerCase().includes(searchQuery.toLowerCase()) ||
          JSON.stringify(log.details).toLowerCase().includes(searchQuery.toLowerCase())
      );
    }

    if (actionFilter !== 'all') {
      filtered = filtered.filter((log) => log.action === actionFilter);
    }

    if (contractFilter !== 'all') {
      filtered = filtered.filter((log) => log.targetContract === contractFilter);
    }

    setFilteredLogs(filtered);
  };

  const exportLogs = () => {
    const dataStr = JSON.stringify(filteredLogs, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `admin-audit-logs-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(url);
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getActionColor = (action: string) => {
    if (action.includes('pause')) return 'text-yellow-500';
    if (action.includes('unpause')) return 'text-green-500';
    if (action.includes('fee')) return 'text-blue-500';
    if (action.includes('dispute')) return 'text-red-500';
    if (action.includes('config')) return 'text-purple-500';
    return 'text-primary';
  };

  const uniqueActions = Array.from(new Set(logs.map((log) => log.action)));

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-2xl font-bold">Audit Logs</h2>
        <button
          onClick={exportLogs}
          className="flex items-center gap-2 px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-border"
        >
          <Download className="w-4 h-4" />
          Export
        </button>
      </div>

      {/* Filters */}
      <div className="mb-6 p-4 bg-card border border-border rounded-lg">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              type="text"
              placeholder="Search logs..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-10 pr-4 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
            />
          </div>

          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-muted-foreground" />
            <select
              value={actionFilter}
              onChange={(e) => setActionFilter(e.target.value)}
              className="flex-1 px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
            >
              <option value="all">All Actions</option>
              {uniqueActions.map((action) => (
                <option key={action} value={action}>
                  {action}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-muted-foreground" />
            <select
              value={contractFilter}
              onChange={(e) => setContractFilter(e.target.value)}
              className="flex-1 px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
            >
              <option value="all">All Contracts</option>
              <option value="atomic_swap">Atomic Swap</option>
              <option value="ip_registry">IP Registry</option>
            </select>
          </div>
        </div>
      </div>

      {/* Logs Table */}
      <div className="bg-card border border-border rounded-lg overflow-hidden">
        {filteredLogs.length === 0 ? (
          <div className="p-8 text-center text-muted-foreground">
            <FileText className="w-12 h-12 mx-auto mb-4 opacity-50" />
            <p>No audit logs found</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-secondary">
                <tr>
                  <th className="px-4 py-3 text-left text-sm font-semibold">Timestamp</th>
                  <th className="px-4 py-3 text-left text-sm font-semibold">Admin</th>
                  <th className="px-4 py-3 text-left text-sm font-semibold">Action</th>
                  <th className="px-4 py-3 text-left text-sm font-semibold">Contract</th>
                  <th className="px-4 py-3 text-left text-sm font-semibold">Details</th>
                </tr>
              </thead>
              <tbody>
                {filteredLogs.map((log) => (
                  <tr key={log.id} className="border-t border-border">
                    <td className="px-4 py-3 text-sm">
                      <div className="flex items-center gap-2">
                        <Clock className="w-4 h-4 text-muted-foreground" />
                        {formatDate(log.timestamp)}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-sm font-mono">
                      {log.admin.slice(0, 8)}...{log.admin.slice(-8)}
                    </td>
                    <td className="px-4 py-3 text-sm font-semibold">
                      <span className={getActionColor(log.action)}>{log.action}</span>
                    </td>
                    <td className="px-4 py-3 text-sm">
                      <span className="px-2 py-1 bg-primary/10 text-primary rounded text-xs">
                        {log.targetContract}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-sm text-muted-foreground">
                      <pre className="text-xs bg-secondary p-2 rounded overflow-x-auto">
                        {JSON.stringify(log.details, null, 2)}
                      </pre>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Stats */}
      <div className="mt-6 grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="p-4 bg-secondary rounded-lg">
          <p className="text-sm text-muted-foreground mb-1">Total Logs</p>
          <p className="text-2xl font-bold">{logs.length}</p>
        </div>
        <div className="p-4 bg-secondary rounded-lg">
          <p className="text-sm text-muted-foreground mb-1">Filtered Results</p>
          <p className="text-2xl font-bold">{filteredLogs.length}</p>
        </div>
        <div className="p-4 bg-secondary rounded-lg">
          <p className="text-sm text-muted-foreground mb-1">Unique Admins</p>
          <p className="text-2xl font-bold">
            {Array.from(new Set(logs.map((log) => log.admin))).length}
          </p>
        </div>
        <div className="p-4 bg-secondary rounded-lg">
          <p className="text-sm text-muted-foreground mb-1">Action Types</p>
          <p className="text-2xl font-bold">{uniqueActions.length}</p>
        </div>
      </div>
    </div>
  );
}

export default AuditLogViewer;

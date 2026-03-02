import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Badge } from "./ui/badge";
import { AlertCircle, Database, TrendingUp, Clock, HardDrive } from "lucide-react";

interface CacheKeyInfo {
  key: string;
  ttl: number;
}

interface CacheStats {
  enabled: boolean;
  uptimeSeconds: number;
  totalKeys: number;
  hits: number;
  misses: number;
  hitRate: number;
  memoryUsed: number;
  memoryUsedHuman: string;
  totalConnections: number;
  totalCommands: number;
  expiredKeys: number;
  evictedKeys: number;
  recentKeys: CacheKeyInfo[];
}

export function CacheStats() {
  const [stats, setStats] = useState<CacheStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStats = async () => {
    try {
      const apiKey = localStorage.getItem("adminApiKey");
      if (!apiKey) {
        setError("未找到 API Key");
        return;
      }

      const response = await fetch("/api/admin/cache/stats", {
        headers: {
          "x-api-key": apiKey,
        },
      });

      if (!response.ok) {
        if (response.status === 500) {
          const data = await response.json();
          if (data.message?.includes("缓存未启用")) {
            setError("缓存未启用");
            return;
          }
        }
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();
      setStats(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "获取缓存统计失败");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 5000); // 每 5 秒刷新
    return () => clearInterval(interval);
  }, []);

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (hours > 0) {
      return `${hours} 小时 ${minutes} 分钟`;
    }
    return `${minutes} 分钟`;
  };

  const formatTTL = (ttl: number) => {
    if (ttl === -1) return "永久";
    if (ttl === -2) return "不存在";
    if (ttl < 60) return `${ttl}秒`;
    const minutes = Math.floor(ttl / 60);
    if (minutes < 60) return `${minutes}分钟`;
    const hours = Math.floor(minutes / 60);
    return `${hours}小时`;
  };

  if (loading) {
    return (
      <div className="space-y-4">
        <div className="h-32 w-full bg-muted animate-pulse rounded-lg" />
        <div className="h-32 w-full bg-muted animate-pulse rounded-lg" />
        <div className="h-32 w-full bg-muted animate-pulse rounded-lg" />
      </div>
    );
  }

  if (error) {
    return (
      <Card className="border-red-200 bg-red-50">
        <CardContent className="pt-6">
          <div className="flex items-center gap-2 text-red-600">
            <AlertCircle className="h-4 w-4" />
            <span>{error}</span>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (!stats) {
    return null;
  }

  const total = stats.hits + stats.misses;

  return (
    <div className="space-y-4">
      {/* 概览卡片 */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">命中率</CardTitle>
            <TrendingUp className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {stats.hitRate.toFixed(1)}%
            </div>
            <p className="text-xs text-muted-foreground">
              {stats.hits} / {total} 次请求
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">缓存数量</CardTitle>
            <Database className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{stats.totalKeys}</div>
            <p className="text-xs text-muted-foreground">
              过期: {stats.expiredKeys} | 驱逐: {stats.evictedKeys}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">运行时间</CardTitle>
            <Clock className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {formatUptime(stats.uptimeSeconds)}
            </div>
            <p className="text-xs text-muted-foreground">
              连接: {stats.totalConnections} | 命令: {stats.totalCommands}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">内存使用</CardTitle>
            <HardDrive className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{stats.memoryUsedHuman}</div>
            <p className="text-xs text-muted-foreground">
              {stats.memoryUsed.toLocaleString()} 字节
            </p>
          </CardContent>
        </Card>
      </div>

      {/* 详细统计 */}
      <Card>
        <CardHeader>
          <CardTitle>缓存详情</CardTitle>
          <CardDescription>
            最近更新: {new Date().toLocaleTimeString()}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <div className="text-sm font-medium text-muted-foreground">
                  缓存命中
                </div>
                <div className="text-2xl font-bold text-green-600">
                  {stats.hits}
                </div>
              </div>
              <div>
                <div className="text-sm font-medium text-muted-foreground">
                  缓存未命中
                </div>
                <div className="text-2xl font-bold text-orange-600">
                  {stats.misses}
                </div>
              </div>
            </div>

            {stats.recentKeys.length > 0 && (
              <div className="mt-6">
                <h4 className="text-sm font-medium mb-2">最近的缓存 Key</h4>
                <div className="space-y-2">
                  {stats.recentKeys.map((keyInfo, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between p-2 rounded-lg bg-muted/50"
                    >
                      <code className="text-xs truncate flex-1 mr-2">
                        {keyInfo.key.substring(0, 60)}...
                      </code>
                      <Badge variant="outline">{formatTTL(keyInfo.ttl)}</Badge>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {total === 0 && (
              <Card className="bg-blue-50 border-blue-200">
                <CardContent className="pt-6">
                  <div className="flex items-center gap-2 text-blue-600">
                    <AlertCircle className="h-4 w-4" />
                    <span>暂无缓存访问记录。缓存功能已启用，等待请求...</span>
                  </div>
                </CardContent>
              </Card>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

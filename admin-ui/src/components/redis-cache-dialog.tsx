import { useEffect, useState } from 'react'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { toast } from 'sonner'
import { Loader2 } from 'lucide-react'
import { getRedisCacheConfig, updateRedisCacheConfig } from '@/api/credentials'

interface RedisCacheDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function RedisCacheDialog({ open, onOpenChange }: RedisCacheDialogProps) {
  const [loading, setLoading] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [enabled, setEnabled] = useState(false)
  const [redisUrl, setRedisUrl] = useState('')

  useEffect(() => {
    if (open) {
      loadCurrentConfig()
    }
  }, [open])

  const loadCurrentConfig = async () => {
    setLoading(true)
    try {
      const data = await getRedisCacheConfig()
      setEnabled(data.enabled)
      setRedisUrl(data.redisUrl || '')
    } catch (error) {
      const message = error instanceof Error ? error.message : '加载配置失败'
      toast.error(`加载配置失败: ${message}`)
      console.error(error)
    } finally {
      setLoading(false)
    }
  }

  const handleSubmit = async () => {
    setSubmitting(true)
    try {
      await updateRedisCacheConfig({
        enabled,
        redisUrl: redisUrl.trim() || null,
      })

      toast.success('Redis 缓存配置已更新')
      onOpenChange(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : '更新配置失败')
      console.error(error)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Redis 缓存配置</DialogTitle>
          <DialogDescription>
            配置 Redis 缓存以提升性能。启用后将使用 Redis 缓存 Token 计算结果。
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin" />
          </div>
        ) : (
          <div className="space-y-4 py-4">
            <div className="flex items-center justify-between">
              <label htmlFor="redis-enabled" className="text-sm font-medium">
                启用 Redis 缓存
              </label>
              <Switch
                id="redis-enabled"
                checked={enabled}
                onCheckedChange={setEnabled}
              />
            </div>

            {enabled && (
              <div className="space-y-2">
                <label htmlFor="redis-url" className="text-sm font-medium">
                  Redis URL
                </label>
                <Input
                  id="redis-url"
                  type="text"
                  placeholder="redis://localhost:6379"
                  value={redisUrl}
                  onChange={(e) => setRedisUrl(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  留空则使用默认配置（redis://localhost:6379）
                </p>
              </div>
            )}
          </div>
        )}

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            取消
          </Button>
          <Button onClick={handleSubmit} disabled={loading || submitting}>
            {submitting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

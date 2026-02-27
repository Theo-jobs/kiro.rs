import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { getGlobalProxy, updateGlobalProxy } from '@/api/credentials'

interface GlobalProxyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function GlobalProxyDialog({ open, onOpenChange }: GlobalProxyDialogProps) {
  const [loading, setLoading] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [proxyUrl, setProxyUrl] = useState('')
  const [proxyUsername, setProxyUsername] = useState('')
  const [proxyPassword, setProxyPassword] = useState('')

  useEffect(() => {
    if (open) {
      loadCurrentConfig()
    }
  }, [open])

  const loadCurrentConfig = async () => {
    setLoading(true)
    try {
      const config = await getGlobalProxy()
      setProxyUrl(config.proxyUrl || '')
      setProxyUsername(config.proxyUsername || '')
      setProxyPassword('')
    } catch (error) {
      toast.error('加载配置失败: ' + (error as Error).message)
    } finally {
      setLoading(false)
    }
  }

  const handleSubmit = async () => {
    setSubmitting(true)
    try {
      await updateGlobalProxy({
        proxyUrl: proxyUrl.trim() || null,
        proxyUsername: proxyUsername.trim() || null,
        proxyPassword: proxyPassword.trim() || null,
      })
      toast.success('全局代理配置已更新')
      onOpenChange(false)
    } catch (error) {
      toast.error('更新失败: ' + (error as Error).message)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>全局代理配置</DialogTitle>
          <DialogDescription>
            配置全局代理，应用于所有未配置凭据级代理的凭据。留空表示不使用代理。
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin" />
          </div>
        ) : (
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <div className="text-sm font-medium">代理地址</div>
              <Input
                id="proxyUrl"
                placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080"
                value={proxyUrl}
                onChange={(e) => setProxyUrl(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                支持 http/https/socks5 协议，留空或填写 "direct" 表示不使用代理
              </p>
            </div>

            <div className="space-y-2">
              <div className="text-sm font-medium">代理用户名（可选）</div>
              <Input
                id="proxyUsername"
                placeholder="留空表示无需认证"
                value={proxyUsername}
                onChange={(e) => setProxyUsername(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <div className="text-sm font-medium">代理密码（可选）</div>
              <Input
                id="proxyPassword"
                type="password"
                placeholder="留空表示无需认证"
                value={proxyPassword}
                onChange={(e) => setProxyPassword(e.target.value)}
              />
            </div>
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

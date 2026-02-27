import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { updateCredentialProxy } from '@/api/credentials'
import { extractErrorMessage } from '@/lib/utils'
import { useQueryClient } from '@tanstack/react-query'

interface EditProxyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  credentialId: number
  currentProxyUrl?: string
}

export function EditProxyDialog({
  open,
  onOpenChange,
  credentialId,
  currentProxyUrl,
}: EditProxyDialogProps) {
  const [proxyUrl, setProxyUrl] = useState('')
  const [proxyUsername, setProxyUsername] = useState('')
  const [proxyPassword, setProxyPassword] = useState('')
  const [saving, setSaving] = useState(false)
  const queryClient = useQueryClient()

  // 当对话框打开时，初始化代理 URL
  useEffect(() => {
    if (open) {
      setProxyUrl(currentProxyUrl || '')
      setProxyUsername('')
      setProxyPassword('')
    }
  }, [open, currentProxyUrl])

  const handleSave = async () => {
    setSaving(true)
    try {
      await updateCredentialProxy(
        credentialId,
        proxyUrl.trim() || null,
        proxyUsername.trim() || null,
        proxyPassword.trim() || null
      )
      toast.success('代理配置已更新')
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
      onOpenChange(false)
    } catch (error) {
      toast.error('更新失败: ' + extractErrorMessage(error))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>编辑代理配置</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <label htmlFor="proxyUrl" className="text-sm font-medium">
              代理 URL
              <span className="text-xs text-muted-foreground ml-2">
                （可选，支持 http/https/socks5，留空使用全局代理，填 "direct" 禁用代理）
              </span>
            </label>
            <Input
              id="proxyUrl"
              placeholder="http://proxy.example.com:8080"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              disabled={saving}
            />
          </div>

          <div className="space-y-2">
            <label htmlFor="proxyUsername" className="text-sm font-medium">
              代理用户名
              <span className="text-xs text-muted-foreground ml-2">（可选）</span>
            </label>
            <Input
              id="proxyUsername"
              placeholder="username"
              value={proxyUsername}
              onChange={(e) => setProxyUsername(e.target.value)}
              disabled={saving}
            />
          </div>

          <div className="space-y-2">
            <label htmlFor="proxyPassword" className="text-sm font-medium">
              代理密码
              <span className="text-xs text-muted-foreground ml-2">（可选）</span>
            </label>
            <Input
              id="proxyPassword"
              type="password"
              placeholder="password"
              value={proxyPassword}
              onChange={(e) => setProxyPassword(e.target.value)}
              disabled={saving}
            />
          </div>
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={saving}
          >
            取消
          </Button>
          <Button type="button" onClick={handleSave} disabled={saving}>
            {saving ? '保存中...' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

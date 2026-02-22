import { useState, useEffect, useCallback, useRef } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { Copy, ExternalLink, Loader2, CheckCircle2, AlertCircle } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { startAuth, getAuthStatus, claimAuth } from '@/api/credentials'
import { extractErrorMessage } from '@/lib/utils'

type Stage = 'input' | 'waiting' | 'success' | 'error'
type AuthMode = 'builder_id' | 'enterprise'

interface EnterpriseLoginDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** 校验 URI 是否为安全的 HTTPS 链接 */
function isSafeUri(uri: string): boolean {
  try {
    const url = new URL(uri)
    return url.protocol === 'https:'
  } catch {
    return false
  }
}

export function EnterpriseLoginDialog({ open, onOpenChange }: EnterpriseLoginDialogProps) {
  const queryClient = useQueryClient()
  const [stage, setStage] = useState<Stage>('input')
  const [mode, setMode] = useState<AuthMode>('builder_id')
  const [startUrl, setStartUrl] = useState('')
  const [region, setRegion] = useState('')
  const [loading, setLoading] = useState(false)
  const [authId, setAuthId] = useState('')
  const [userCode, setUserCode] = useState('')
  const [verificationUri, setVerificationUri] = useState('')
  const [countdown, setCountdown] = useState(0)
  const [credentialId, setCredentialId] = useState<number | null>(null)
  const [errorMsg, setErrorMsg] = useState('')

  // 互斥锁：防止倒计时和轮询竞态修改 stage
  const resolvedRef = useRef(false)

  // 关闭时重置状态
  const reset = useCallback(() => {
    setStage('input')
    setMode('builder_id')
    setStartUrl('')
    setRegion('')
    setLoading(false)
    setAuthId('')
    setUserCode('')
    setVerificationUri('')
    setCountdown(0)
    setCredentialId(null)
    setErrorMsg('')
    resolvedRef.current = false
  }, [])

  useEffect(() => {
    if (!open) reset()
  }, [open, reset])

  // 倒计时（不依赖 countdown，避免每秒重建 interval）
  useEffect(() => {
    if (stage !== 'waiting') return
    const timer = setInterval(() => {
      setCountdown(prev => {
        if (prev <= 1) {
          clearInterval(timer)
          if (!resolvedRef.current) {
            resolvedRef.current = true
            setErrorMsg('验证超时，请重试')
            setStage('error')
          }
          return 0
        }
        return prev - 1
      })
    }, 1000)
    return () => clearInterval(timer)
  }, [stage])

  // 轮询认证状态
  useEffect(() => {
    if (stage !== 'waiting' || !authId) return
    let cancelled = false
    const poll = setInterval(async () => {
      try {
        const res = await getAuthStatus(authId)
        if (cancelled || resolvedRef.current) return
        if (res.status === 'completed') {
          resolvedRef.current = true
          clearInterval(poll)
          try {
            const claimed = await claimAuth(authId, {})
            if (cancelled) return
            setCredentialId(claimed.credentialId)
            setStage('success')
            queryClient.invalidateQueries({ queryKey: ['credentials'] })
            toast.success('认证成功，凭据已添加')
          } catch (err) {
            if (cancelled) return
            setErrorMsg(extractErrorMessage(err))
            setStage('error')
          }
        } else if (res.status === 'failed') {
          resolvedRef.current = true
          clearInterval(poll)
          setErrorMsg(res.error || '认证失败')
          setStage('error')
        }
      } catch {
        // 轮询失败静默忽略，等待下次
      }
    }, 5000)
    return () => {
      cancelled = true
      clearInterval(poll)
    }
  }, [stage, authId, queryClient])

  const handleStart = async () => {
    setLoading(true)
    resolvedRef.current = false
    try {
      const res = await startAuth({
        mode,
        ...(mode === 'enterprise' ? { startUrl: startUrl || undefined, region: region || undefined } : {}),
      })
      setAuthId(res.authId)
      setUserCode(res.userCode)
      setVerificationUri(res.verificationUri)
      setCountdown(res.expiresIn)
      setStage('waiting')
    } catch (err) {
      setErrorMsg(extractErrorMessage(err))
      setStage('error')
    } finally {
      setLoading(false)
    }
  }

  const handleCopyCode = async () => {
    try {
      await navigator.clipboard.writeText(userCode)
      toast.success('用户码已复制')
    } catch {
      toast.error('复制失败')
    }
  }

  const formatCountdown = (s: number) => {
    const m = Math.floor(s / 60)
    const sec = s % 60
    return `${m}:${sec.toString().padStart(2, '0')}`
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {stage === 'input' && 'OIDC 认证登录'}
            {stage === 'waiting' && '等待认证'}
            {stage === 'success' && '认证成功'}
            {stage === 'error' && '认证失败'}
          </DialogTitle>
          <DialogDescription>
            {stage === 'input' && '选择认证模式并开始登录流程'}
            {stage === 'waiting' && '请在浏览器中完成认证'}
            {stage === 'success' && '凭据已成功添加'}
            {stage === 'error' && '认证过程中出现错误'}
          </DialogDescription>
        </DialogHeader>

        {stage === 'input' && (
          <div className="space-y-4">
            <div className="flex gap-2" role="group" aria-label="认证模式选择">
              <Button
                variant={mode === 'builder_id' ? 'default' : 'outline'}
                onClick={() => setMode('builder_id')}
                aria-pressed={mode === 'builder_id'}
                className="flex-1"
                size="sm"
              >
                Builder ID
              </Button>
              <Button
                variant={mode === 'enterprise' ? 'default' : 'outline'}
                onClick={() => setMode('enterprise')}
                aria-pressed={mode === 'enterprise'}
                className="flex-1"
                size="sm"
              >
                Enterprise
              </Button>
            </div>
            {mode === 'enterprise' && (
              <div className="space-y-3">
                <div className="space-y-1.5">
                  <label htmlFor="oidc-start-url" className="text-sm font-medium">Start URL</label>
                  <Input
                    id="oidc-start-url"
                    placeholder="https://view.awsapps.com/start"
                    value={startUrl}
                    onChange={e => setStartUrl(e.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <label htmlFor="oidc-region" className="text-sm font-medium">Region</label>
                  <Input
                    id="oidc-region"
                    placeholder="us-east-1"
                    value={region}
                    onChange={e => setRegion(e.target.value)}
                  />
                </div>
              </div>
            )}
            <DialogFooter>
              <Button onClick={handleStart} disabled={loading}>
                {loading && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                开始认证
              </Button>
            </DialogFooter>
          </div>
        )}
        {stage === 'waiting' && (
          <div className="space-y-4">
            <div className="text-center space-y-3">
              <div className="text-sm text-muted-foreground">请复制以下用户码并在浏览器中输入</div>
              <div className="flex items-center justify-center gap-2">
                <code className="font-mono text-3xl tracking-widest font-bold" aria-label="用户验证码">{userCode}</code>
                <Button variant="ghost" size="icon" onClick={handleCopyCode} aria-label="复制用户码">
                  <Copy className="h-4 w-4" />
                </Button>
              </div>
              {isSafeUri(verificationUri) ? (
                <div className="flex items-center justify-center gap-2">
                  <a
                    href={verificationUri}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm text-blue-600 hover:underline flex items-center gap-1"
                  >
                    打开验证页面 <ExternalLink className="h-3 w-3" />
                  </a>
                </div>
              ) : (
                <div className="text-sm text-muted-foreground text-center">
                  验证链接: {verificationUri}
                </div>
              )}
              <div className="text-sm text-muted-foreground">
                剩余时间: {formatCountdown(countdown)}
              </div>
              <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" />
                等待认证完成...
              </div>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
            </DialogFooter>
          </div>
        )}
        {stage === 'success' && (
          <div className="space-y-4">
            <div className="text-center space-y-3">
              <CheckCircle2 className="h-12 w-12 text-green-500 mx-auto" />
              <div className="text-sm">
                凭据 ID: <span className="font-bold">#{credentialId}</span>
              </div>
            </div>
            <DialogFooter>
              <Button onClick={() => onOpenChange(false)}>关闭</Button>
            </DialogFooter>
          </div>
        )}

        {stage === 'error' && (
          <div className="space-y-4">
            <div className="text-center space-y-3">
              <AlertCircle className="h-12 w-12 text-red-500 mx-auto" />
              <div className="text-sm text-red-600">{errorMsg}</div>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>关闭</Button>
              <Button onClick={reset}>重试</Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

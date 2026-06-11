import {
  Box,
  CircularProgress,
  List,
  ListItem,
  ListItemText,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material'
import { useLockFn } from 'ahooks'
import { useImperativeHandle, useState, type Ref } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, DialogRef } from '@/components/base'
import {
  getProfiles,
  importProfile,
  nodeAuthGetDeviceFp,
  nodeAuthGetStatus,
  nodeAuthLogin,
  nodeAuthLogout,
  nodeAuthRegister,
  updateProfile,
} from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

type AuthMode = 'login' | 'register'

interface Props {
  ref?: Ref<DialogRef>
  onChanged?: (status: INodeAuthStatus) => void
}

export function NodeAuthViewer({ ref, onChanged }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [isWorking, setIsWorking] = useState(false)
  const [mode, setMode] = useState<AuthMode>('login')

  const [status, setStatus] = useState<INodeAuthStatus | null>(null)
  const [deviceFp, setDeviceFp] = useState('')
  const [server, setServer] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  const refresh = async () => {
    const [st, fp] = await Promise.all([
      nodeAuthGetStatus(),
      nodeAuthGetDeviceFp(),
    ])
    setStatus(st)
    setDeviceFp(fp)
    setServer(st.server || '')
    setUsername(st.username || '')
    return st
  }

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true)
      setMode('login')
      setPassword('')
      refresh().catch((err) => console.error('[NodeAuthViewer] refresh', err))
    },
    close: () => setOpen(false),
  }))

  const validateInputs = (requireEmail: boolean) => {
    if (!server.trim()) {
      showNotice.error('settings.sections.nodeAuth.messages.serverRequired')
      return false
    }
    const name = username.trim()
    if (!name) {
      showNotice.error(
        requireEmail
          ? 'settings.sections.nodeAuth.messages.emailRequired'
          : 'settings.sections.nodeAuth.messages.usernameRequired',
      )
      return false
    }
    if (requireEmail && !name.includes('@')) {
      showNotice.error('settings.sections.nodeAuth.messages.emailInvalid')
      return false
    }
    if (!password) {
      showNotice.error('settings.sections.nodeAuth.messages.passwordRequired')
      return false
    }
    return true
  }

  // 登录成功后，自动把该用户的固定订阅链接导入为订阅（已存在则刷新）。
  const autoImportSubscription = async (url: string) => {
    if (!url) return
    try {
      const profiles = await getProfiles()
      const existing = profiles.items?.find((it) => it.url === url)
      if (existing?.uid) {
        await updateProfile(existing.uid)
      } else {
        await importProfile(url)
      }
      showNotice.success(
        'settings.sections.nodeAuth.messages.subscriptionImported',
      )
    } catch (err) {
      showNotice.error(
        'settings.sections.nodeAuth.messages.subscriptionImportFailed',
        err,
        4000,
      )
    }
  }

  const onLogin = useLockFn(async () => {
    if (!validateInputs(false)) return
    try {
      setIsWorking(true)
      const st = await nodeAuthLogin(server.trim(), username.trim(), password)
      setStatus(st)
      setPassword('')
      onChanged?.(st)
      showNotice.success('settings.sections.nodeAuth.messages.loginSuccess')
      setOpen(false)
      await autoImportSubscription(st.subscription_url)
    } catch (err) {
      showNotice.error(
        'settings.sections.nodeAuth.messages.loginFailed',
        err,
        4000,
      )
    } finally {
      setIsWorking(false)
    }
  })

  const onRegister = useLockFn(async () => {
    if (!validateInputs(true)) return
    try {
      setIsWorking(true)
      const res = await nodeAuthRegister(
        server.trim(),
        username.trim(),
        password,
      )
      setPassword('')
      // 服务端 message 为后端动态文案，优先展示；否则回退到本地化提示。
      showNotice.success(
        res.message || t('settings.sections.nodeAuth.messages.registerSuccess'),
        undefined,
        5000,
      )
      setMode('login')
    } catch (err) {
      showNotice.error(
        'settings.sections.nodeAuth.messages.registerFailed',
        err,
        5000,
      )
    } finally {
      setIsWorking(false)
    }
  })

  const onLogout = useLockFn(async () => {
    try {
      setIsWorking(true)
      await nodeAuthLogout()
      const st = await refresh()
      onChanged?.(st)
      showNotice.success('settings.sections.nodeAuth.messages.logoutSuccess')
    } catch (err) {
      showNotice.error(
        'shared.feedback.notifications.common.saveFailed',
        err,
        4000,
      )
    } finally {
      setIsWorking(false)
    }
  })

  const loggedIn = !!status?.logged_in
  const isRegister = mode === 'register'

  const okLabel = isWorking
    ? t('shared.statuses.saving')
    : isRegister
      ? t('settings.sections.nodeAuth.actions.register')
      : loggedIn
        ? t('settings.sections.nodeAuth.actions.relogin')
        : t('settings.sections.nodeAuth.actions.login')

  return (
    <BaseDialog
      open={open}
      title={t('settings.sections.nodeAuth.title')}
      contentSx={{ width: 420 }}
      okBtn={
        isWorking ? (
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <CircularProgress size={16} color="inherit" />
            {okLabel}
          </Box>
        ) : (
          okLabel
        )
      }
      cancelBtn={t('shared.actions.cancel')}
      disableOk={isWorking}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={isRegister ? onRegister : onLogin}
    >
      <Tabs
        value={mode}
        onChange={(_, v: AuthMode) => setMode(v)}
        variant="fullWidth"
        sx={{ mb: 1, minHeight: 36 }}
      >
        <Tab
          value="login"
          label={t('settings.sections.nodeAuth.tabs.login')}
          disabled={isWorking}
          sx={{ minHeight: 36 }}
        />
        <Tab
          value="register"
          label={t('settings.sections.nodeAuth.tabs.register')}
          disabled={isWorking}
          sx={{ minHeight: 36 }}
        />
      </Tabs>

      <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
        {t('settings.sections.nodeAuth.description')}
      </Typography>

      <List sx={{ py: 0 }}>
        <ListItem sx={{ px: 0, py: 0.5 }}>
          <ListItemText
            primary={t('settings.sections.nodeAuth.deviceFp')}
            secondary={
              <Box component="span" sx={{ wordBreak: 'break-all' }}>
                {deviceFp || '-'}
              </Box>
            }
          />
        </ListItem>
        {loggedIn && !isRegister && (
          <>
            <ListItem sx={{ px: 0, py: 0.5 }}>
              <ListItemText
                primary={t('settings.sections.nodeAuth.accountExpiresAt')}
                secondary={
                  status?.account_expires_at ||
                  t('settings.sections.nodeAuth.noExpiry')
                }
              />
            </ListItem>
            <ListItem sx={{ px: 0, py: 0.5 }}>
              <ListItemText
                primary={t('settings.sections.nodeAuth.expiresAt')}
                secondary={
                  status?.expired
                    ? `${status?.expires_at || '-'} (${t('settings.sections.nodeAuth.expired')})`
                    : status?.expires_at || '-'
                }
              />
            </ListItem>
            <ListItem sx={{ px: 0, py: 0.5 }}>
              <ListItemText
                primary={t('settings.sections.nodeAuth.devices')}
                secondary={`${status?.active_devices ?? '-'} / ${status?.max_devices ?? '-'}`}
              />
            </ListItem>
            {!!status?.subscription_url && (
              <ListItem sx={{ px: 0, py: 0.5 }}>
                <ListItemText
                  primary={t('settings.sections.nodeAuth.subscription')}
                  secondary={
                    <Box component="span" sx={{ wordBreak: 'break-all' }}>
                      {status.subscription_url}
                    </Box>
                  }
                />
              </ListItem>
            )}
          </>
        )}
      </List>

      <TextField
        fullWidth
        size="small"
        sx={{ mt: 1 }}
        label={t('settings.sections.nodeAuth.fields.server')}
        placeholder={t('settings.sections.nodeAuth.placeholders.server')}
        value={server}
        onChange={(e) => setServer(e.target.value)}
        disabled={isWorking}
      />
      <TextField
        fullWidth
        size="small"
        sx={{ mt: 1.5 }}
        label={t(
          isRegister
            ? 'settings.sections.nodeAuth.fields.email'
            : 'settings.sections.nodeAuth.fields.username',
        )}
        placeholder={t(
          isRegister
            ? 'settings.sections.nodeAuth.placeholders.email'
            : 'settings.sections.nodeAuth.placeholders.username',
        )}
        value={username}
        onChange={(e) => setUsername(e.target.value)}
        disabled={isWorking}
      />
      <TextField
        fullWidth
        size="small"
        type="password"
        sx={{ mt: 1.5 }}
        label={t('settings.sections.nodeAuth.fields.password')}
        placeholder={t('settings.sections.nodeAuth.placeholders.password')}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        disabled={isWorking}
      />

      {loggedIn && !isRegister && (
        <Box sx={{ mt: 2, display: 'flex', justifyContent: 'flex-end' }}>
          <Typography
            component="button"
            variant="body2"
            color="error"
            onClick={onLogout}
            sx={{
              background: 'none',
              border: 'none',
              cursor: isWorking ? 'default' : 'pointer',
              padding: 0,
            }}
          >
            {t('settings.sections.nodeAuth.actions.logout')}
          </Typography>
        </Box>
      )}
    </BaseDialog>
  )
}

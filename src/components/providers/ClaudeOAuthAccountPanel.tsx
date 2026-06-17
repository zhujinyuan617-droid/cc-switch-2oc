import { useState } from "react";
import { Download, Loader2, Plus, User, X, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useClaudeOauth } from "@/components/providers/forms/hooks";

export function ClaudeOAuthAccountPanel() {
  const { t } = useTranslation();
  const [label, setLabel] = useState("");

  const {
    accounts,
    hasAnyAccount,
    defaultAccountId,
    isImportingCurrentAccount,
    importCurrentAccountAsync,
    isApplyingAccount,
    applyAccountAsync,
    isRemovingAccount,
    removeAccount,
  } = useClaudeOauth();

  const handleImportCurrent = async () => {
    try {
      const account = await importCurrentAccountAsync(label);
      setLabel("");
      toast.success(
        t("claudeOauth.importSuccess", {
          defaultValue: "已导入当前 Claude Code 官方登录态",
        }),
      );

      // 导入后立即应用，使用户能直接完成“登录 → 导入 → 使用”。
      await applyAccountAsync(account.id);
      toast.success(
        t("claudeOauth.applySuccess", {
          account: account.login,
          defaultValue: `已切换到 ${account.login}`,
        }),
      );
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleApply = async (accountId: string, login: string) => {
    try {
      await applyAccountAsync(accountId);
      toast.success(
        t("claudeOauth.applySuccess", {
          account: login,
          defaultValue: `已切换到 ${login}`,
        }),
      );
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRemove = (accountId: string) => {
    removeAccount(accountId);
  };

  return (
    <section className="rounded-lg border bg-card p-4 shadow-sm">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold">
              {t("claudeOauth.managerTitle", {
                defaultValue: "Claude 官方账号切换",
              })}
            </h3>
            <Badge variant={hasAnyAccount ? "default" : "secondary"}>
              {hasAnyAccount
                ? t("claudeOauth.accountCount", {
                    count: accounts.length,
                    defaultValue: `${accounts.length} 个账号`,
                  })
                : t("claudeOauth.notImported", "未导入")}
            </Badge>
          </div>
          <p className="text-xs leading-relaxed text-muted-foreground">
            {t("claudeOauth.managerHint", {
              defaultValue:
                "先用 Claude Code 登录目标官方账号，再导入当前登录态。点击切换会直接写回 ~/.claude/.credentials.json；建议切换前关闭 Claude Code。",
            })}
          </p>
        </div>
      </div>

      {hasAnyAccount && (
        <div className="mt-4 space-y-2">
          {accounts.map((account) => {
            const isDefault = defaultAccountId === account.id;
            return (
              <div
                key={account.id}
                className="flex flex-col gap-2 rounded-md border bg-muted/30 p-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="flex min-w-0 items-center gap-2">
                  <User className="h-5 w-5 shrink-0 text-muted-foreground" />
                  <span className="truncate text-sm font-medium">
                    {account.login}
                  </span>
                  {isDefault && (
                    <Badge variant="secondary" className="shrink-0 text-xs">
                      {t("claudeOauth.defaultAccount", "默认")}
                    </Badge>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-2">
                  <Button
                    type="button"
                    size="sm"
                    variant={isDefault ? "secondary" : "default"}
                    onClick={() => handleApply(account.id, account.login)}
                    disabled={isApplyingAccount}
                  >
                    {isApplyingAccount ? (
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    ) : isDefault ? (
                      <Check className="mr-2 h-4 w-4" />
                    ) : null}
                    {isDefault
                      ? t("claudeOauth.reapplyAccount", {
                          defaultValue: "重新应用",
                        })
                      : t("claudeOauth.switchToAccount", {
                          defaultValue: "切换",
                        })}
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    className="h-8 w-8 text-muted-foreground hover:text-red-500"
                    onClick={() => handleRemove(account.id)}
                    disabled={isRemovingAccount || isApplyingAccount}
                    title={t("claudeOauth.removeAccount", "移除账号")}
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <div className="mt-4 grid gap-2 sm:grid-cols-[1fr_auto] sm:items-end">
        <div className="space-y-2">
          <Label className="text-sm text-muted-foreground">
            {t("claudeOauth.importLabel", "导入标签，可选")}
          </Label>
          <Input
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            placeholder={t("claudeOauth.importLabelPlaceholder", {
              defaultValue: "例如 Claude Max A",
            })}
          />
        </div>
        <Button
          type="button"
          variant="outline"
          onClick={handleImportCurrent}
          disabled={isImportingCurrentAccount || isApplyingAccount}
        >
          {isImportingCurrentAccount ? (
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          ) : hasAnyAccount ? (
            <Plus className="mr-2 h-4 w-4" />
          ) : (
            <Download className="mr-2 h-4 w-4" />
          )}
          {t("claudeOauth.importCurrent", "导入当前 Claude Code 登录态")}
        </Button>
      </div>
    </section>
  );
}

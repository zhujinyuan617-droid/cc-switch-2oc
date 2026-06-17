import { useMutation, useQueryClient } from "@tanstack/react-query";
import { authApi } from "@/lib/api";
import { useManagedAuth } from "./useManagedAuth";

export function useClaudeOauth() {
  const managedAuth = useManagedAuth("claude_oauth");
  const queryClient = useQueryClient();
  const queryKey = ["managed-auth-status", "claude_oauth"];

  const importCurrentMutation = useMutation({
    mutationFn: (label?: string) =>
      authApi.authImportCurrentClaudeAccount(label),
    onSuccess: async () => {
      await managedAuth.refetchStatus();
      await queryClient.invalidateQueries({ queryKey });
    },
  });

  const applyAccountMutation = useMutation({
    mutationFn: (accountId: string) =>
      authApi.authApplyClaudeAccount(accountId),
    onSuccess: async () => {
      await managedAuth.refetchStatus();
      await queryClient.invalidateQueries({ queryKey });
    },
  });

  return {
    ...managedAuth,
    importCurrentAccount: importCurrentMutation.mutate,
    importCurrentAccountAsync: importCurrentMutation.mutateAsync,
    isImportingCurrentAccount: importCurrentMutation.isPending,
    importCurrentError: importCurrentMutation.error,
    applyAccount: applyAccountMutation.mutate,
    applyAccountAsync: applyAccountMutation.mutateAsync,
    isApplyingAccount: applyAccountMutation.isPending,
    applyAccountError: applyAccountMutation.error,
  };
}

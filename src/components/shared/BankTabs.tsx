import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
import type { Bank } from "../../types/models";
import { useT } from "../../i18n/LanguageContext";

interface BankTabsProps {
  banks: Bank[];
  activeBankId: string;
  onChanged: () => void;
}

function BankTabs({ banks, activeBankId, onChanged }: BankTabsProps) {
  const t = useT();
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newBankName, setNewBankName] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [error, setError] = useState("");

  const sorted = [...banks].sort((a, b) => a.order - b.order);

  async function handleSwitch(bankId: string) {
    if (bankId === activeBankId) return;
    try {
      await invoke("set_active_bank", { bankId });
      onChanged();
    } catch (err) {
      setError(t.bankTabs.switchError(`${err}`));
    }
  }

  async function handleCreate() {
    if (!newBankName.trim()) return;
    try {
      await invoke("create_bank", { name: newBankName.trim() });
      setNewBankName("");
      setCreating(false);
      onChanged();
    } catch (err) {
      setError(t.bankTabs.createError(`${err}`));
    }
  }

  async function commitRename(bankId: string) {
    if (!renameValue.trim()) {
      setRenamingId(null);
      return;
    }
    try {
      await invoke("rename_bank", { bankId, name: renameValue.trim() });
      setRenamingId(null);
      onChanged();
    } catch (err) {
      setError(t.bankTabs.renameError(`${err}`));
    }
  }

  async function handleDelete(bankId: string) {
    try {
      await invoke("delete_bank", { bankId });
      setConfirmingDeleteId(null);
      onChanged();
    } catch (err) {
      setError(t.bankTabs.deleteError(`${err}`));
    }
  }

  async function handleDrop() {
    if (!dragId || !dropTargetId || dragId === dropTargetId) {
      setDragId(null);
      setDropTargetId(null);
      return;
    }
    const ids = sorted.map((b) => b.id);
    const fromIdx = ids.indexOf(dragId);
    const toIdx = ids.indexOf(dropTargetId);
    ids.splice(toIdx, 0, ids.splice(fromIdx, 1)[0]);

    setDragId(null);
    setDropTargetId(null);
    try {
      await invoke("reorder_banks", { orderedBankIds: ids });
      onChanged();
    } catch (err) {
      setError(t.bankTabs.reorderError(`${err}`));
    }
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center gap-1">
        {sorted.map((bank) => (
          <div
            key={bank.id}
            draggable
            onDragStart={() => setDragId(bank.id)}
            onDragOver={(e) => {
              e.preventDefault();
              if (bank.id !== dragId) setDropTargetId(bank.id);
            }}
            onDrop={handleDrop}
            onDragEnd={() => {
              setDragId(null);
              setDropTargetId(null);
            }}
            className={`group flex items-center gap-1 rounded-t px-3 py-1.5 text-sm ${
              bank.id === activeBankId
                ? "bg-neutral-300 text-neutral-900"
                : "bg-neutral-900 text-neutral-400 hover:bg-neutral-800"
            } ${dropTargetId === bank.id ? "outline outline-1 outline-accent" : ""}`}
          >
            {renamingId === bank.id ? (
              <input
                autoFocus
                className="w-24 rounded bg-neutral-950 px-1 text-neutral-100"
                value={renameValue}
                onChange={(e) => setRenameValue(e.currentTarget.value)}
                onBlur={() => commitRename(bank.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitRename(bank.id);
                  if (e.key === "Escape") setRenamingId(null);
                }}
              />
            ) : (
              <button
                onClick={() => handleSwitch(bank.id)}
                onDoubleClick={() => {
                  setRenamingId(bank.id);
                  setRenameValue(bank.name);
                }}
                title={t.bankTabs.switchRenameTitle}
                className="font-medium"
              >
                {bank.name}
              </button>
            )}

            {confirmingDeleteId === bank.id ? (
              <span className="flex items-center gap-1 text-xs">
                <button className="text-red-300 hover:text-red-100" onClick={() => handleDelete(bank.id)}>
                  {t.common.yes}
                </button>
                <button className="text-neutral-400 hover:text-neutral-200" onClick={() => setConfirmingDeleteId(null)}>
                  {t.common.no}
                </button>
              </span>
            ) : (
              <button
                onClick={() => setConfirmingDeleteId(bank.id)}
                className="hidden text-xs text-neutral-400 hover:text-red-300 group-hover:inline"
                title={t.bankTabs.deleteTitle}
              >
                ×
              </button>
            )}
          </div>
        ))}

        {creating ? (
          <input
            autoFocus
            className="w-28 rounded bg-neutral-900 px-2 py-1.5 text-sm text-neutral-100"
            placeholder={t.bankTabs.namePlaceholder}
            value={newBankName}
            onChange={(e) => setNewBankName(e.currentTarget.value)}
            onBlur={handleCreate}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreate();
              if (e.key === "Escape") setCreating(false);
            }}
          />
        ) : (
          <button
            onClick={() => setCreating(true)}
            className="flex items-center gap-1 rounded px-2 py-1.5 text-sm text-neutral-500 hover:bg-neutral-800 hover:text-neutral-300"
            title={t.bankTabs.newBankTitle}
          >
            <Plus className="h-3.5 w-3.5" />
            {t.bankTabs.newBank}
          </button>
        )}
      </div>
      {error && <p className="text-xs text-red-400">{error}</p>}
    </div>
  );
}

export default BankTabs;

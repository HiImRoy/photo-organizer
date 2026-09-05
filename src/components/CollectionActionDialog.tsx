import { useEffect, useMemo, useRef, useState } from "react";

import type { CollectionSummary } from "../types";

export type CollectionDialogRequest =
  | { kind: "add-assets"; assetIds: number[] }
  | { kind: "move-assets"; assetIds: number[]; sourceCollectionId: number }
  | { kind: "rename"; collection: CollectionSummary }
  | { kind: "move-collection"; collection: CollectionSummary }
  | { kind: "delete"; collection: CollectionSummary; hasChildren: boolean };

export type CollectionDialogSubmission =
  | { kind: "add-assets"; collectionIds: number[]; assetIds: number[] }
  | {
      kind: "move-assets";
      sourceCollectionId: number;
      targetCollectionId: number;
      assetIds: number[];
    }
  | { kind: "rename"; collectionId: number; name: string }
  | {
      kind: "move-collection";
      collectionId: number;
      parentCollectionId: number | null;
    }
  | {
      kind: "delete";
      collectionId: number;
      mode: "deleteSubtree" | "promoteChildren";
    };

interface CollectionActionDialogProps {
  request: CollectionDialogRequest;
  collections: CollectionSummary[];
  busy: boolean;
  onClose: () => void;
  onSubmit: (submission: CollectionDialogSubmission) => void;
}

export function CollectionActionDialog({
  request,
  collections,
  busy,
  onClose,
  onSubmit,
}: CollectionActionDialogProps) {
  const dialogRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return undefined;
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusableElements = () =>
      Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
        ),
      ).filter((element) => !element.hasAttribute("hidden"));
    const initialFocus =
      dialog.querySelector<HTMLElement>(
        "input:not(:disabled), select:not(:disabled), textarea:not(:disabled)",
      ) ?? focusableElements()[0];
    const focusFrame = window.requestAnimationFrame(() => initialFocus?.focus());
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = focusableElements();
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    dialog.addEventListener("keydown", handleKeyDown);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      dialog.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [onClose]);
  const [selectedCollectionIds, setSelectedCollectionIds] = useState<number[]>([]);
  const [targetCollectionId, setTargetCollectionId] = useState<number | null>(null);
  const [name, setName] = useState(request.kind === "rename" ? request.collection.name : "");
  const [parentCollectionId, setParentCollectionId] = useState<number | null>(
    request.kind === "move-collection" ? request.collection.parentCollectionId : null,
  );
  const [deleteMode, setDeleteMode] = useState<"deleteSubtree" | "promoteChildren">(
    "promoteChildren",
  );

  const manualCollections = useMemo(
    () => collections.filter((collection) => collection.collectionKind === "manual"),
    [collections],
  );

  const allowedParentCollections = useMemo(() => {
    if (request.kind !== "move-collection") return manualCollections;
    return manualCollections.filter(
      (candidate) =>
        candidate.id !== request.collection.id &&
        !isCollectionDescendant(candidate.id, request.collection.id, collections),
    );
  }, [collections, manualCollections, request]);

  const title =
    request.kind === "add-assets"
      ? "加入收藏"
      : request.kind === "move-assets"
        ? "移动到收藏夹"
        : request.kind === "rename"
          ? "重命名收藏夹"
          : request.kind === "move-collection"
            ? "移动收藏夹"
            : "删除收藏夹";

  const submit = () => {
    if (request.kind === "add-assets") {
      onSubmit({
        kind: "add-assets",
        collectionIds: selectedCollectionIds,
        assetIds: request.assetIds,
      });
      return;
    }
    if (request.kind === "move-assets" && targetCollectionId !== null) {
      onSubmit({
        kind: "move-assets",
        sourceCollectionId: request.sourceCollectionId,
        targetCollectionId,
        assetIds: request.assetIds,
      });
      return;
    }
    if (request.kind === "rename" && name.trim()) {
      onSubmit({ kind: "rename", collectionId: request.collection.id, name: name.trim() });
      return;
    }
    if (request.kind === "move-collection") {
      onSubmit({
        kind: "move-collection",
        collectionId: request.collection.id,
        parentCollectionId,
      });
      return;
    }
    if (request.kind === "delete") {
      onSubmit({
        kind: "delete",
        collectionId: request.collection.id,
        mode: request.hasChildren ? deleteMode : "deleteSubtree",
      });
    }
  };

  const submitDisabled =
    busy ||
    (request.kind === "add-assets" && selectedCollectionIds.length === 0) ||
    (request.kind === "move-assets" && targetCollectionId === null) ||
    (request.kind === "rename" && !name.trim());

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={dialogRef}
        className="collection-action-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="collection-action-dialog-title"
      >
        <div className="dialog-heading">
          <h2 id="collection-action-dialog-title">{title}</h2>
          <button type="button" className="dialog-close" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>

        {request.kind === "add-assets" ? (
          <>
            <p className="collection-dialog-count">{request.assetIds.length} 张图片</p>
            <div className="collection-choice-list" role="group" aria-label="目标收藏夹">
              {collections.map((collection) => {
                const checked = selectedCollectionIds.includes(collection.id);
                return (
                  <label key={collection.id} className="collection-choice">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() =>
                        setSelectedCollectionIds((current) =>
                          checked
                            ? current.filter((id) => id !== collection.id)
                            : [...current, collection.id],
                        )
                      }
                    />
                    <span>{collection.name}</span>
                    <small>{collection.assetCount}</small>
                  </label>
                );
              })}
            </div>
          </>
        ) : null}

        {request.kind === "move-assets" ? (
          <>
            <p className="collection-dialog-count">{request.assetIds.length} 张图片</p>
            <label className="collection-dialog-field">
              <span>目标收藏夹</span>
              <select
                aria-label="目标收藏夹"
                value={targetCollectionId ?? ""}
                onChange={(event) =>
                  setTargetCollectionId(event.target.value ? Number(event.target.value) : null)
                }
              >
                <option value="">请选择</option>
                {collections
                  .filter((collection) => collection.id !== request.sourceCollectionId)
                  .map((collection) => (
                    <option key={collection.id} value={collection.id}>
                      {collection.name}
                    </option>
                  ))}
              </select>
            </label>
          </>
        ) : null}

        {request.kind === "rename" ? (
          <label className="collection-dialog-field">
            <span>名称</span>
            <input
              aria-label="收藏夹名称"
              value={name}
              maxLength={100}
              autoFocus
              onChange={(event) => setName(event.target.value)}
            />
          </label>
        ) : null}

        {request.kind === "move-collection" ? (
          <label className="collection-dialog-field">
            <span>放入</span>
            <select
              aria-label="父收藏夹"
              value={parentCollectionId ?? ""}
              onChange={(event) =>
                setParentCollectionId(event.target.value ? Number(event.target.value) : null)
              }
            >
              <option value="">顶层收藏夹</option>
              {allowedParentCollections.map((collection) => (
                <option key={collection.id} value={collection.id}>
                  {collection.name}
                </option>
              ))}
            </select>
          </label>
        ) : null}

        {request.kind === "delete" ? (
          <div className="collection-delete-options">
            <strong>{request.collection.name}</strong>
            {request.hasChildren ? (
              <>
                <label>
                  <input
                    type="radio"
                    name="collection-delete-mode"
                    checked={deleteMode === "promoteChildren"}
                    onChange={() => setDeleteMode("promoteChildren")}
                  />
                  <span>删除当前收藏夹，保留子收藏夹</span>
                </label>
                <label>
                  <input
                    type="radio"
                    name="collection-delete-mode"
                    checked={deleteMode === "deleteSubtree"}
                    onChange={() => setDeleteMode("deleteSubtree")}
                  />
                  <span>删除整个收藏夹树</span>
                </label>
              </>
            ) : (
              <span>只删除收藏关系，不会删除照片。</span>
            )}
          </div>
        ) : null}

        <div className="dialog-actions">
          <button type="button" className="tool-button" disabled={busy} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className={request.kind === "delete" ? "danger-primary-action" : "primary-action"}
            disabled={submitDisabled}
            onClick={submit}
          >
            {busy ? "处理中" : request.kind === "delete" ? "删除" : "确定"}
          </button>
        </div>
      </section>
    </div>
  );
}

function isCollectionDescendant(
  candidateId: number,
  ancestorId: number,
  collections: CollectionSummary[],
): boolean {
  const byId = new Map(collections.map((collection) => [collection.id, collection]));
  let current = byId.get(candidateId)?.parentCollectionId ?? null;
  while (current !== null) {
    if (current === ancestorId) return true;
    current = byId.get(current)?.parentCollectionId ?? null;
  }
  return false;
}

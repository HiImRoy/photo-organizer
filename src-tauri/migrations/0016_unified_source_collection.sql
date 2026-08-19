-- 0053: separate physical Source ownership from virtual Collection membership.
-- Data conversion is completed by migrate_unified_source_collection in db.rs
-- inside the same transaction as this schema migration.

ALTER TABLE collection_assets RENAME TO collection_assets_legacy_0016;
ALTER TABLE collections RENAME TO collections_legacy_0016;

CREATE TABLE collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    parent_collection_id INTEGER REFERENCES collections(id) ON DELETE CASCADE,
    collection_kind TEXT NOT NULL DEFAULT 'manual'
        CHECK (collection_kind IN ('manual', 'system_favorites')),
    system_key TEXT,
    display_order INTEGER NOT NULL DEFAULT 0
);

INSERT INTO collections(
    id, name, description, created_at, updated_at,
    parent_collection_id, collection_kind, system_key, display_order
)
SELECT
    id, name, description, created_at, updated_at,
    NULL, 'manual', NULL, 0
FROM collections_legacy_0016;

CREATE TABLE collection_assets (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY(collection_id, asset_id)
);

INSERT INTO collection_assets(collection_id, asset_id, added_at)
SELECT collection_id, asset_id, added_at
FROM collection_assets_legacy_0016;

DROP TABLE collection_assets_legacy_0016;
DROP TABLE collections_legacy_0016;

CREATE INDEX idx_collection_assets_asset
    ON collection_assets(asset_id, collection_id);

CREATE UNIQUE INDEX idx_collections_root_name
    ON collections(name COLLATE NOCASE)
    WHERE parent_collection_id IS NULL;

CREATE UNIQUE INDEX idx_collections_parent_name
    ON collections(parent_collection_id, name COLLATE NOCASE)
    WHERE parent_collection_id IS NOT NULL;

CREATE UNIQUE INDEX idx_collections_system_key
    ON collections(system_key)
    WHERE system_key IS NOT NULL;

CREATE INDEX idx_collections_parent_order
    ON collections(parent_collection_id, display_order, id);

CREATE INDEX idx_collection_assets_collection
    ON collection_assets(collection_id, asset_id);

CREATE TRIGGER collections_reject_system_parent_insert
BEFORE INSERT ON collections
WHEN NEW.parent_collection_id IS NOT NULL
 AND EXISTS(
     SELECT 1 FROM collections
     WHERE id=NEW.parent_collection_id AND collection_kind='system_favorites'
 )
BEGIN
    SELECT RAISE(ABORT, 'system favorites cannot have children');
END;

CREATE TRIGGER collections_reject_system_node_parent
BEFORE INSERT ON collections
WHEN NEW.collection_kind='system_favorites' AND NEW.parent_collection_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'system favorites must be a root node');
END;

CREATE TRIGGER collections_reject_system_parent_update
BEFORE UPDATE OF parent_collection_id, collection_kind ON collections
WHEN (NEW.collection_kind='system_favorites' AND NEW.parent_collection_id IS NOT NULL)
   OR (NEW.parent_collection_id IS NOT NULL AND EXISTS(
       SELECT 1 FROM collections
       WHERE id=NEW.parent_collection_id AND collection_kind='system_favorites'
   ))
BEGIN
    SELECT RAISE(ABORT, 'system favorites must remain a root leaf');
END;

CREATE TRIGGER collections_reject_system_delete
BEFORE DELETE ON collections
WHEN OLD.system_key='default_favorites'
BEGIN
    SELECT RAISE(ABORT, 'default favorites cannot be deleted');
END;

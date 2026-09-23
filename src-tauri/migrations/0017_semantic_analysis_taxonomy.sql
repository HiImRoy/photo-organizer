ALTER TABLE semantic_embeddings
    ADD COLUMN taxonomy_version TEXT NOT NULL DEFAULT '';

UPDATE semantic_embeddings AS embedding
SET taxonomy_version = COALESCE(
    (
        SELECT evidence.taxonomy_version
        FROM semantic_evidence evidence
        WHERE evidence.asset_id = embedding.asset_id
          AND evidence.model_name = embedding.model_name
          AND evidence.model_version = embedding.model_version
          AND evidence.analysis_version = embedding.analysis_version
          AND evidence.source_fingerprint = embedding.source_fingerprint
        ORDER BY evidence.generated_at DESC, evidence.rank ASC
        LIMIT 1
    ),
    (
        SELECT labels.taxonomy_version
        FROM semantic_labels labels
        WHERE labels.asset_id = embedding.asset_id
          AND labels.model_name = embedding.model_name
          AND labels.model_version = embedding.model_version
          AND labels.analysis_version = embedding.analysis_version
          AND labels.source_fingerprint = embedding.source_fingerprint
        ORDER BY labels.generated_at DESC, labels.id DESC
        LIMIT 1
    ),
    ''
);

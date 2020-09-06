ALTER TABLE trees
    ALTER COLUMN species TYPE Smallint
    USING species::integer
;

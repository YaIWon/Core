#!/bin/bash
# Run this periodically to update dashboard data
# Can be triggered by GitHub Actions

cat > dashboard/data.json << EOF
{
  "vector_count": $(ls -1 data/vectors/ 2>/dev/null | wc -l),
  "block_height": $(cat data/blockchain/chain.json 2>/dev/null | jq '. | length' || echo 0),
  "files_processed": $(find training_data -type f 2>/dev/null | wc -l),
  "recent_learning": $(tail -20 data/blockchain/chain.json 2>/dev/null | jq '[.[] | {time: .timestamp, content: .content_preview}]' || echo '[]'),
  "last_updated": "$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
}
EOF

echo "Dashboard data updated"

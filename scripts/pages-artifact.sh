#!/bin/sh
# Make the GitHub Pages deployment reliable around the "github-pages" artifact.
#
#   pages-artifact.sh clean   before actions/upload-pages-artifact
#   pages-artifact.sh wait    after it, before actions/deploy-pages
#
# actions/deploy-pages looks the artifact up by name in this workflow run and fails when it
# finds none or more than one. GitHub can list a freshly uploaded artifact a little late, and
# a re-run attempt keeps the artifacts of earlier attempts. "clean" deletes those stale ones;
# "wait" waits until exactly one is listed. Needs GH_TOKEN with actions: write.
set -eu

ids() {
    gh api "repos/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}/artifacts?name=github-pages" \
        --jq '.artifacts[].id'
}

case "${1:-}" in
clean)
    for id in $(ids); do
        echo "Deleting github-pages artifact $id from an earlier attempt"
        gh api -X DELETE "repos/${GITHUB_REPOSITORY}/actions/artifacts/$id"
    done
    ;;
wait)
    attempt=0
    while :; do
        count=$(ids | wc -l)
        if [ "$count" -eq 1 ]; then
            echo "The github-pages artifact is listed"
            exit 0
        fi
        attempt=$((attempt + 1))
        if [ "$attempt" -ge 30 ]; then
            echo "::error::Expected one github-pages artifact in this run, found $count after 5 minutes"
            exit 1
        fi
        echo "Found $count github-pages artifact(s); waiting for GitHub to list the upload"
        sleep 10
    done
    ;;
*)
    echo "usage: $0 clean|wait" >&2
    exit 2
    ;;
esac

#!/usr/bin/env bash
# One-time Google Cloud setup for continuous deployment of Plug-Fest Bot.
# Creates the runtime and deployer service accounts, grants the deployer what it
# needs, and connects Cloud Build to GitHub (opens a browser sign-in).
set -euo pipefail

PROJECT=plug-fest-bot
REGION=us-central1
CONNECTION=cognitive-proof-github
RUN_SA=plug-fest-bot-run@$PROJECT.iam.gserviceaccount.com
DEPLOY_SA=plug-fest-bot-deployer@$PROJECT.iam.gserviceaccount.com

gcloud config set project "$PROJECT"

gcloud iam service-accounts describe "$RUN_SA" >/dev/null 2>&1 ||
  gcloud iam service-accounts create plug-fest-bot-run --display-name="Plug-Fest Bot Cloud Run runtime"
gcloud iam service-accounts describe "$DEPLOY_SA" >/dev/null 2>&1 ||
  gcloud iam service-accounts create plug-fest-bot-deployer --display-name="Plug-Fest Bot Cloud Build deployer"

for role in roles/run.admin roles/artifactregistry.writer roles/logging.logWriter; do
  gcloud projects add-iam-policy-binding "$PROJECT" --member="serviceAccount:$DEPLOY_SA" --role="$role" --condition=None --format=none
done
gcloud iam service-accounts add-iam-policy-binding "$RUN_SA" --member="serviceAccount:$DEPLOY_SA" --role=roles/iam.serviceAccountUser --format=none

# Prints a link: open it, sign in to GitHub, and install the Cloud Build app on
# the Cognitive-Proof org with access to the plug-fest-bot repository.
gcloud builds connections create github "$CONNECTION" --region="$REGION"

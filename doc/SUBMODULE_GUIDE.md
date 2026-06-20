# Git Submodule Guide for XCPlite

This document provides instructions for managing the XCPlite git submodule in this repository.

## Overview

This repository uses the XCPlite library as a git submodule located in the `xcplib/` directory.

- **Submodule URL**: https://github.com/RainerZ/XCPlite.git
- **Current Branch**: V2.0.0
- **Local Directory**: `xcplib/`

## Table of Contents

1. [Initial Setup - Creating the Submodule](#initial-setup---creating-the-submodule)
2. [Cloning a Repository with Submodules](#cloning-a-repository-with-submodules)
3. [Updating Submodules](#updating-submodules)
4. [Switching Branches](#switching-branches)
5. [Switching to a Different Commit/Tag](#switching-to-a-different-committag)
6. [Changing the Submodule URL](#changing-the-submodule-url)
7. [Removing a Submodule](#removing-a-submodule)
8. [Common Issues and Troubleshooting](#common-issues-and-troubleshooting)

---

## Initial Setup - Creating the Submodule

The submodule has already been created in this repository. Here's how it was done for future reference:

```bash
# Add the submodule with a specific branch
git submodule add -b V2.0.0 https://github.com/RainerZ/XCPlite.git xcplib

# Commit the changes
git commit -m "Add XCPlite submodule at V2.0.0"
```

This command:
- Adds the XCPlite repository as a submodule
- Tracks the `V2.0.0` branch
- Places it in the `xcplib/` directory
- Creates/updates `.gitmodules` file
- Creates a reference to the specific commit in the parent repository

---

## Cloning a Repository with Submodules

When someone else clones this repository, they need to initialize the submodules:

### Option 1: Clone with submodules in one command
```bash
git clone --recurse-submodules https://github.com/your-repo/UDPlog.git
```

### Option 2: Clone first, then initialize submodules
```bash
# Clone the main repository
git clone https://github.com/your-repo/UDPlog.git
cd UDPlog

# Initialize and clone the submodules
git submodule init
git submodule update
```

### Option 3: Combined command
```bash
git clone https://github.com/your-repo/UDPlog.git
cd UDPlog
git submodule update --init --recursive
```

---

## Updating Submodules

### Update to Latest Commit on Tracked Branch

To update the submodule to the latest commit on its tracked branch (V2.0.0):

```bash
# Navigate to the submodule directory
cd xcplib

# Fetch and merge the latest changes from the tracked branch
git pull origin V2.1.5

# Go back to the parent repository
cd ..

# Stage the submodule update
git add xcplib

# Commit the update
git commit -m "Update xcplib submodule to latest V2.1.5"
```

### Update All Submodules at Once

From the parent repository root:

```bash
# Update all submodules to the latest commit on their tracked branches
git submodule update --remote

# Stage and commit the changes
git add .
git commit -m "Update all submodules"
```

### Update to Specific Commit in Parent Repository

If you've pulled changes in the parent repository that reference a different submodule commit:

```bash
# Update submodules to match the parent repository's references
git submodule update --init --recursive
```

---

## Switching Branches

### Switch the Submodule to a Different Branch

```bash
# Navigate to the submodule directory
cd xcplib

git fetch


# Checkout the desired branch
git checkout V2.0.0

# Pull the latest changes on that branch
git pull origin V2.0.0

# Go back to the parent repository
cd ..

# Update .gitmodules to track the new branch (optional but recommended)
git config -f .gitmodules submodule.xcplib.branch V2.0.0

# Stage the changes
git add xcplib .gitmodules

# Commit the branch switch
git commit -m "Switch xcplib submodule to V2.0.0 branch"
```

### Configure Submodule to Always Track a Branch

```bash
# Set the branch in .gitmodules
git config -f .gitmodules submodule.xcplib.branch V2.0.0

# Commit the configuration change
git add .gitmodules
git commit -m "Configure xcplib to track V2.0.0 branch"
```

---

## Switching to a Different Commit/Tag

### Checkout a Specific Tag

```bash
# Navigate to the submodule directory
cd xcplib

# Fetch all tags
git fetch --tags

# Checkout the specific tag
git checkout tags/V2.0.0

# Go back to the parent repository
cd ..

# Stage and commit
git add xcplib
git commit -m "Pin xcplib submodule to tag V2.0.0"
```

### Checkout a Specific Commit Hash

```bash
# Navigate to the submodule directory
cd xcplib

# Checkout the specific commit
git checkout 0c1bb547ff241d32efe49f58ffdb2e9d1b541dff

# Go back to the parent repository
cd ..

# Stage and commit
git add xcplib
git commit -m "Pin xcplib to specific commit"
```

---

## Changing the Submodule URL

If the submodule repository moves to a different URL or you want to use a different fork:

```bash
# Update the URL in .gitmodules
git config -f .gitmodules submodule.xcplib.url https://github.com/RainerZ/XCPlite.git
# Sync the configuration to .git/config
git submodule sync

# Update the submodule with the new URL
git submodule update --init --recursive --remote

# Commit the change
git add .gitmodules
git commit -m "Update xcplib submodule URL"
```

---

## Removing a Submodule

If you need to remove the submodule completely:

```bash
# Remove the submodule entry from .gitmodules
git config -f .gitmodules --remove-section submodule.xcplib

# Remove the submodule entry from .git/config
git config -f .git/config --remove-section submodule.xcplib

# Remove the submodule directory from the index
git rm --cached xcplib

# Remove the submodule directory from the working tree
rm -rf xcplib

# Remove the submodule metadata
rm -rf .git/modules/xcplib

# Commit the removal
git add .gitmodules
git commit -m "Remove xcplib submodule"
```

---

## Common Issues and Troubleshooting

### Submodule Directory is Empty

```bash
# Initialize and update the submodule
git submodule update --init --recursive
```

### Submodule Shows as Modified (Dirty)

This happens when you have uncommitted changes in the submodule or are on a different commit than referenced by the parent repository.

```bash
# Check what's different
cd xcplib
git status

# Option 1: Discard changes and reset to parent's reference
cd ..
git submodule update --force

# Option 2: Commit changes in submodule and update parent reference
cd xcplib
git add .
git commit -m "Changes in submodule"
cd ..
git add xcplib
git commit -m "Update submodule reference"
```

### Detached HEAD in Submodule

Submodules are often in "detached HEAD" state (not on a branch). This is normal when pinned to a specific commit.

```bash
# To work on a branch instead:
cd xcplib
git checkout V2.0.0
git pull origin V2.0.0
```

### Merge Conflicts in Submodule References

When merging branches that have different submodule commits:

```bash
# Accept their version
git checkout --theirs xcplib
git add xcplib

# Or accept your version
git checkout --ours xcplib
git add xcplib

# Then update the submodule to match
git submodule update --init
```

### Submodule Update Failed

```bash
# Force fetch and reset
cd xcplib
git fetch origin
git reset --hard origin/V2.0.0
cd ..
```

---

## Quick Reference Commands

```bash
# Check submodule status
git submodule status

# Initialize submodules after clone
git submodule update --init --recursive

# Update submodules to latest on tracked branch
git submodule update --remote

# Update submodules to parent's referenced commit
git submodule update

# View submodule configuration
cat .gitmodules
git config --list | grep submodule

# Execute git command in all submodules
git submodule foreach 'git pull origin V2.0.0'

# Show diff including submodule changes
git diff --submodule

# Configure git to show submodule diffs inline
git config --global diff.submodule log
```

---

## Best Practices

1. **Always commit submodule updates** in the parent repository after updating the submodule
2. **Document the reason** for switching branches or updating submodules in commit messages
3. **Use branch tracking** (configured in .gitmodules) when you want to follow the latest changes
4. **Pin to specific commits/tags** for stable releases to ensure reproducible builds
5. **Communicate with team** when updating submodules, as everyone will need to run `git submodule update`
6. **Check submodule status** before committing to avoid accidentally committing wrong references
7. **Use `--recurse-submodules`** flag with git commands when appropriate (clone, pull, etc.)

---

## Additional Resources

- [Git Submodules Documentation](https://git-scm.com/book/en/v2/Git-Tools-Submodules)
- [GitHub Submodules Guide](https://github.blog/2016-02-01-working-with-submodules/)
- [XCPlite Repository](https://github.com/RainerZ/XCPlite)

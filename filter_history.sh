git filter-branch -f --msg-filter 'sed "s/Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>//g"' HEAD~8..HEAD

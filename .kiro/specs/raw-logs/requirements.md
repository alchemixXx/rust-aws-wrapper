# Requirements Document

## Introduction

This feature adds a `raw logs` command to the CLI tool that fetches CloudWatch log events from a specified log group within a time range and saves them to a `logs.json` file in the current working directory. The command prompts the user for an AWS profile (role), authenticates via SSO, then collects log group name, start time, and end time before fetching and persisting the results.

## Glossary

- **CLI**: The `raw` command-line application built in Rust
- **Log_Fetcher**: The module responsible for calling AWS CloudWatch Logs and retrieving log events
- **Profile_Selector**: The interactive prompt that reads `~/.aws/config` and presents available AWS profiles as a selectable list to the user
- **SSO_Authenticator**: The existing SSO login module (`AwsSso`) that generates temporary credentials for a given profile
- **AWS_Config_File**: The file located at `~/.aws/config` that contains AWS profile definitions in INI format with `[profile <name>]` headers
- **Log_Group**: An AWS CloudWatch Logs log group selected from the list of available log groups retrieved via `describe-log-groups` (e.g., `/aws/lambda/my-function`)
- **Start_Time**: The beginning of the time range for log retrieval, provided as an ISO 8601 timestamp or relative expression
- **End_Time**: The end of the time range for log retrieval, provided as an ISO 8601 timestamp or relative expression

## Requirements

### Requirement 1: Register the Raw Logs Command

**User Story:** As a developer, I want a `raw logs` subcommand available in the CLI, so that I can fetch CloudWatch logs without remembering the full AWS CLI syntax.

#### Acceptance Criteria

1. WHEN the user runs `raw logs`, THE CLI SHALL start the raw logs interactive flow by presenting the first prompt of the log retrieval workflow.
2. WHEN the user runs `raw --help`, THE CLI SHALL list `logs` as an available subcommand with a description of no more than 80 characters summarizing its purpose.
3. IF the user runs `raw` without specifying a subcommand, THEN THE CLI SHALL display the help text for the `raw` command listing all available subcommands.
4. IF the user runs `raw logs` with an unrecognized flag or argument, THEN THE CLI SHALL display an error message indicating the invalid usage and exit with a non-zero exit code.

### Requirement 2: Profile Selection

**User Story:** As a developer, I want to choose which AWS profile (role) to use from my existing AWS configuration, so that I can fetch logs from the correct account and environment without manually typing profile names.

#### Acceptance Criteria

1. WHEN the `raw logs` command is invoked, THE Profile_Selector SHALL read the `~/.aws/config` file and parse all profile names defined with `[profile <name>]` headers
2. WHEN profiles are parsed, THE Profile_Selector SHALL display the profile names as an interactive selectable list for the user to choose from
3. WHEN the user selects a profile, THE SSO_Authenticator SHALL authenticate using the selected profile and set temporary credentials as environment variables for the current process
4. IF the SSO authentication fails, THEN THE CLI SHALL display an error message indicating the authentication failure reason and terminate the command with a non-zero exit code
5. IF the `~/.aws/config` file does not exist or is not readable, THEN THE CLI SHALL display an error message indicating the config file could not be read and terminate the command with a non-zero exit code
6. IF no profiles are found in the `~/.aws/config` file, THEN THE CLI SHALL display an error message indicating no profiles are available and terminate the command with a non-zero exit code

### Requirement 3: Log Group Selection and Time Range

**User Story:** As a developer, I want to select a log group from the available groups in my account and provide a time range interactively, so that I can specify exactly which logs to retrieve without memorizing log group names.

#### Acceptance Criteria

1. WHEN SSO authentication succeeds, THE Log_Fetcher SHALL call AWS CloudWatch Logs `describe-log-groups` to retrieve the list of available log groups for the authenticated profile
2. WHEN log groups are retrieved, THE CLI SHALL display the log group names as an interactive selectable list for the user to choose from
3. WHEN the user selects a log group, THE CLI SHALL prompt the user to enter a start time in ISO 8601 format (e.g., `2024-01-15T10:00:00Z`)
4. WHEN the user provides a start time, THE CLI SHALL prompt the user to enter an end time in ISO 8601 format
5. IF the `describe-log-groups` call fails due to insufficient permissions, THEN THE CLI SHALL display an error message indicating the permissions issue and terminate the command with a non-zero exit code
6. IF no log groups are found for the authenticated profile, THEN THE CLI SHALL display a message indicating no log groups were found and terminate the command with a non-zero exit code
7. IF the user provides a time value that does not conform to ISO 8601 format, THEN THE CLI SHALL display the expected format with an example and re-prompt
8. IF the user provides an end time that is earlier than or equal to the start time, THEN THE CLI SHALL display a validation error indicating the end time must be after the start time and re-prompt for the end time

### Requirement 4: Fetch Logs from CloudWatch

**User Story:** As a developer, I want the tool to fetch all log events within my specified time range, so that I can analyze them locally.

#### Acceptance Criteria

1. WHEN valid log group name, start time, and end time are provided, THE Log_Fetcher SHALL call AWS CloudWatch Logs `filter-log-events` with the log group name, start time, and end time as parameters
2. WHILE fetching logs, THE Log_Fetcher SHALL follow pagination tokens to retrieve all matching events until no further pagination token is returned or a timeout of 300 seconds for the overall fetch operation is reached
3. IF the log group does not exist, THEN THE Log_Fetcher SHALL display an error message indicating the log group was not found and terminate the command with a non-zero exit code
4. IF the AWS API returns an access denied error, THEN THE CLI SHALL display a permissions error and terminate the command with a non-zero exit code
5. IF the AWS API returns a network error or throttling response, THEN THE Log_Fetcher SHALL retry the request up to 3 times with exponential backoff starting at 1 second, and if all retries fail, display an error message indicating the failure reason and terminate the command with a non-zero exit code

### Requirement 5: Save Logs to File

**User Story:** As a developer, I want logs saved to a JSON file in my current directory, so that I can search, filter, and share them easily.

#### Acceptance Criteria

1. WHEN log events are successfully fetched, THE CLI SHALL write the complete response to a file named `logs.json` in the current working directory
2. WHEN writing the file, THE CLI SHALL format the JSON with 2-space indentation
3. IF a `logs.json` file already exists in the current directory, THEN THE CLI SHALL overwrite the existing file without prompting
4. IF writing to the file fails, THEN THE CLI SHALL display an error message indicating the cause of failure and terminate with a non-zero exit code
5. WHEN the file is written successfully, THE CLI SHALL display the number of events saved and the absolute file path
6. IF the fetch succeeds but returns zero log events, THEN THE CLI SHALL write an empty JSON array to `logs.json` and display a message indicating that no events were found for the given time range

### Requirement 6: JSON Output Structure

**User Story:** As a developer, I want the output file to contain structured log event data, so that I can parse it programmatically.

#### Acceptance Criteria

1. THE CLI SHALL write log events as a JSON array of objects, where each object represents one log event
2. WHEN writing each log event, THE CLI SHALL include exactly three fields: `timestamp` (number), `message` (string), and `logStreamName` (string)
3. WHEN writing timestamps, THE CLI SHALL preserve the original millisecond-precision Unix timestamp from AWS as a numeric value
4. IF a log event returned from AWS contains an empty or null message field, THEN THE CLI SHALL include the event with an empty string as the message value

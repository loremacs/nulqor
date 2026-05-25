<#
.SYNOPSIS
  Interact with a running Nulqor app over HTTP (port 8080).
.DESCRIPTION
  Wrapper for the Phase 2 HTTP API (decisions/006). Use when the Tauri app is running
  (npm start). External agents must register an observer before sending messages.
.PARAMETER Action
  health | connect | models | register | send | transcript | catch-up | ack | observers
.PARAMETER BaseUrl
  API base URL. Default: http://127.0.0.1:8080 or NULQOR_API_URL env var.
.PARAMETER Message
  Text for Action=send.
.PARAMETER ObserverName
  Observer name for register/send/catch-up/ack. Default for send: cursor-agent.
.PARAMETER Url
  LM Studio URL for Action=connect. Default: http://localhost:1234/v1
.PARAMETER Model
  Optional model id override for Action=send.
.PARAMETER Agent
  Optional agent persona for Action=send.
.PARAMETER WaitSeconds
  After send, poll transcript until assistant reply or timeout. Default: 30.
.PARAMETER AutoAck
  For catch-up: advance ack pointer after returning events.
.PARAMETER Quiet
  Suppress OK output on success (still prints message content for send).
#>
param(
    [ValidateSet("health", "connect", "models", "register", "send", "transcript", "catch-up", "ack", "observers")]
    [string]$Action = "send",

    [string]$BaseUrl = "",

    [string]$Message = "",

    [string]$ObserverName = "cursor-agent",

    [string]$Url = "http://localhost:1234/v1",

    [string]$Model = "",

    [string]$Agent = "",

    [int]$WaitSeconds = 30,

    [switch]$AutoAck,

    [switch]$Quiet
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
    $BaseUrl = if ($env:NULQOR_API_URL) { $env:NULQOR_API_URL.TrimEnd('/') } else { "http://127.0.0.1:8080" }
}

function Invoke-NulqorGet {
    param([string]$Path)
    Invoke-RestMethod -Uri ($BaseUrl + $Path) -Method Get
}

function Invoke-NulqorPost {
    param([string]$Path, [object]$Body)
    $json = if ($Body -is [string]) { $Body } else { $Body | ConvertTo-Json -Depth 10 -Compress }
    Invoke-RestMethod -Uri ($BaseUrl + $Path) -Method Post -ContentType "application/json" -Body $json
}

function Write-Result {
    param($Obj)
    if (-not $Quiet) {
        $Obj | ConvertTo-Json -Depth 10
    }
}

switch ($Action) {
    "health" {
        Write-Result (Invoke-NulqorGet "/health")
    }

    "connect" {
        Write-Result (Invoke-NulqorPost "/connect" @{ url = $Url })
    }

    "models" {
        Write-Result (Invoke-NulqorGet "/models")
    }

    "register" {
        Write-Result (Invoke-NulqorPost "/observers/register" @{ name = $ObserverName })
    }

    "observers" {
        Write-Result (Invoke-NulqorGet "/observers")
    }

    "catch-up" {
        $aa = if ($AutoAck) { "true" } else { "false" }
        $enc = [uri]::EscapeDataString($ObserverName)
        Write-Result (Invoke-NulqorGet "/observers/catch-up?observer=$enc&auto_ack=$aa")
    }

    "ack" {
        Write-Result (Invoke-NulqorPost "/observers/ack" @{ name = $ObserverName })
    }

    "transcript" {
        Write-Result (Invoke-NulqorGet "/transcript")
    }

    "send" {
        if ([string]::IsNullOrWhiteSpace($Message)) {
            Write-Host "FAIL: -Message required for Action=send"
            exit 1
        }

        # Ensure observer exists (idempotent register)
        try {
            Invoke-NulqorPost "/observers/register" @{ name = $ObserverName } | Out-Null
        }
        catch {
            Write-Host "FAIL: could not register observer - is the app running? $_"
            exit 1
        }

        $body = @{
            message       = $Message
            observer_name = $ObserverName
        }
        if ($Model) { $body.model = $Model }
        if ($Agent) { $body.agent = $Agent }

        $stream = Invoke-NulqorPost "/message" $body
        if (-not $Quiet) {
            Write-Host "stream_id: $($stream.stream_id)"
        }

        $deadline = (Get-Date).AddSeconds($WaitSeconds)
        $before = (Invoke-NulqorGet "/transcript").messages.Count

        while ((Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 500
            $t = Invoke-NulqorGet "/transcript"
            if ($t.messages.Count -gt $before) {
                $last = $t.messages[-1]
                if ($last.role -eq "assistant") {
                    Write-Host ""
                    Write-Host ("[" + $last.role + "] " + $last.participant_name)
                    Write-Host $last.content
                    if (-not $Quiet) {
                        Write-Host ""
                        Write-Host "latency_ms: $($last.latency_ms)  tokens: $($last.tokens)"
                    }
                    exit 0
                }
            }
        }

        Write-Host "FAIL: no assistant reply within ${WaitSeconds}s (user message may have been added)"
        Invoke-NulqorGet "/transcript" | ConvertTo-Json -Depth 6
        exit 1
    }
}

if (-not $Quiet) {
    Write-Host "OK: $Action"
}
exit 0

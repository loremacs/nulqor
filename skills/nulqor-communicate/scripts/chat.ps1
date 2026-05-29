<#
.SYNOPSIS
  Interact with a running Nulqor app over HTTP (port 8080).
.DESCRIPTION
  Wrapper for the Phase 2 HTTP API (decisions/006). Use when the Tauri app is running
  (npm start). External agents must register an observer before sending messages.
.PARAMETER Action
  health | ready | connect | select-model | models | register | send | transcript | catch-up | ack | observers
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
.PARAMETER SkipProviderCheck
  For Action=send only: skip LM Studio / active-model preflight (not recommended).
#>
param(
    [ValidateSet("health", "ready", "connect", "select-model", "models", "register", "send", "transcript", "catch-up", "ack", "observers")]
    [string]$Action = "send",

    [string]$BaseUrl = "",

    [string]$Message = "",

    [string]$ObserverName = "cursor-agent",

    [string]$Url = "http://localhost:1234/v1",

    [string]$Model = "",

    [string]$Agent = "",

    [int]$WaitSeconds = 30,

    [switch]$AutoAck,

    [switch]$Quiet,

    [switch]$SkipProviderCheck
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

function Get-ProviderStatus {
    param(
        [string]$RequiredModel = "",
        [switch]$RequireApp
    )

    $status = [ordered]@{
        ok              = $false
        app             = $false
        provider_ready  = $false
        reason          = ""
        hint            = ""
        active          = $null
        models          = @()
        model_count     = 0
        model           = $null
    }

    try {
        $health = Invoke-NulqorGet "/health"
        if (-not $health.ok) {
            $status.reason = "app_unhealthy"
            $status.hint = "Nulqor /health returned ok=false. Restart: npm start"
            return $status
        }
        $status.app = $true
    }
    catch {
        $status.reason = "app_unreachable"
        $status.hint = "Start the Nulqor app (npm start). Connection refused at $BaseUrl"
        return $status
    }

    try {
        $modelsResp = Invoke-NulqorGet "/models?refresh=true"
        if ($modelsResp.models) {
            $status.models = @($modelsResp.models)
        }
        $status.model_count = $status.models.Count
        $status.active = $modelsResp.active
    }
    catch {
        $status.reason = "provider_unreachable"
        $status.hint = "LM Studio not reachable. Start LM Studio, load a model, then:`n  chat.ps1 -Action connect -Url $Url`nOr click Connect in the chat-panel."
        return $status
    }

    if ($status.model_count -eq 0) {
        $status.reason = "no_models_loaded"
        $status.hint = "LM Studio returned no models. Load a model in LM Studio, then:`n  chat.ps1 -Action connect -Url $Url"
        return $status
    }

    $modelToUse = if ($RequiredModel) { $RequiredModel } else { $status.active }
    if ([string]::IsNullOrWhiteSpace($modelToUse)) {
        $status.reason = "no_active_model"
        $status.hint = "Provider is not connected in the app (no active model). Click Connect in chat-panel or run:`n  chat.ps1 -Action connect -Url $Url"
        return $status
    }

    if ($RequiredModel -and ($RequiredModel -notin $status.models)) {
        $status.reason = "model_not_found"
        $status.hint = "Model '$RequiredModel' is not in the provider list. Run chat.ps1 -Action models"
        return $status
    }

    $status.model = $modelToUse
    $status.provider_ready = $true
    $status.ok = $true
    return $status
}

function Assert-ProviderReady {
    param([string]$RequiredModel = "")

    $status = Get-ProviderStatus -RequiredModel $RequiredModel
    if ($status.ok) {
        return $status
    }

    Write-Host "FAIL: provider not ready ($($status.reason))"
    if ($status.hint) {
        Write-Host $status.hint
    }
    if (-not $Quiet) {
        $status | ConvertTo-Json -Depth 6
    }
    exit 1
}

switch ($Action) {
    "health" {
        Write-Result (Invoke-NulqorGet "/health")
    }

    "ready" {
        $status = Get-ProviderStatus -RequiredModel $Model
        Write-Result $status
        if (-not $status.ok) { exit 1 }
    }

    "connect" {
        Write-Result (Invoke-NulqorPost "/connect" @{ url = $Url })
    }

    "select-model" {
        if ([string]::IsNullOrWhiteSpace($Model)) {
            Write-Host "FAIL: -Model required for Action=select-model"
            exit 1
        }
        Write-Result (Invoke-NulqorPost "/select-model" @{ model = $Model })
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

        if (-not $SkipProviderCheck) {
            Assert-ProviderReady -RequiredModel $Model | Out-Null
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

        Write-Host "FAIL: no assistant reply within ${WaitSeconds}s"
        if (-not $SkipProviderCheck) {
            Write-Host "Hint: run chat.ps1 -Action ready - provider may have disconnected or generation failed."
        }
        Invoke-NulqorGet "/transcript" | ConvertTo-Json -Depth 6
        exit 1
    }
}

if (-not $Quiet) {
    Write-Host "OK: $Action"
}
exit 0

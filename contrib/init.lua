---@diagnostic disable: undefined-global
local bin = vim.fn.getcwd() .. '/target/debug/bls'

vim.diagnostic.config({ virtual_text = true, signs = true, underline = true, update_in_insert = true })
vim.opt.completeopt = { 'menu', 'menuone', 'noselect' }
vim.keymap.set('i', '<Tab>',   function() return vim.snippet.active({ direction =  1 }) and '<cmd>lua vim.snippet.jump(1)<cr>'  or '<Tab>'   end, { expr = true })
vim.keymap.set('i', '<S-Tab>', function() return vim.snippet.active({ direction = -1 }) and '<cmd>lua vim.snippet.jump(-1)<cr>' or '<S-Tab>' end, { expr = true })

vim.api.nvim_create_autocmd('LspAttach', {
  callback = function(ev)
    local client = vim.lsp.get_client_by_id(ev.data.client_id)
    if not client then return end
    vim.lsp.completion.enable(true, client.id, ev.buf, { autotrigger = false })
    vim.api.nvim_create_autocmd('TextChangedI', {
      buffer = ev.buf,
      callback = function()
        local col = vim.api.nvim_win_get_cursor(0)[2]
        local char = vim.api.nvim_get_current_line():sub(col, col)
        if char:match('[%w%$%{%-_]') then
          vim.lsp.completion.get()
        end
      end,
    })
  end,
})

vim.lsp.config('bls', {
  cmd = { bin },
  filetypes = { 'sh' },
  root_markers = { '.git' },
})
vim.lsp.enable('bls')

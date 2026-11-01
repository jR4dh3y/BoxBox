package authcontext

import "context"

type usernameKey struct{}
type clientIDKey struct{}
type accountSessionKey struct{}

// WithAccountSession marks a validated account JWT, excluding local dev identities.
func WithAccountSession(ctx context.Context) context.Context {
	return context.WithValue(ctx, accountSessionKey{}, true)
}

func IsAccountSession(ctx context.Context) bool {
	account, _ := ctx.Value(accountSessionKey{}).(bool)
	return account && ClientID(ctx) == ""
}

func WithClientID(ctx context.Context, id string) context.Context {
	return context.WithValue(ctx, clientIDKey{}, id)
}

func ClientID(ctx context.Context) string {
	id, _ := ctx.Value(clientIDKey{}).(string)
	return id
}

func WithUsername(ctx context.Context, username string) context.Context {
	return context.WithValue(ctx, usernameKey{}, username)
}

func Username(ctx context.Context) string {
	username, _ := ctx.Value(usernameKey{}).(string)
	return username
}

// Long-lived streams recheck authorization before releasing another event.
type authorizationKey struct{}

func WithAuthorization(ctx context.Context, valid func() bool) context.Context {
	return context.WithValue(ctx, authorizationKey{}, valid)
}
func Authorized(ctx context.Context) bool {
	valid, ok := ctx.Value(authorizationKey{}).(func() bool)
	return !ok || valid()
}
